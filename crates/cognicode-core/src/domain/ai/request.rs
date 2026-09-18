//! Investigation request — what is sent to an `LlmPort`.
//!
//! The request is **structured**, not a freeform prompt. It carries:
//! - the frame (immutable input),
//! - the tool/read declarations the agent is being asked about,
//! - the bounded textual instruction,
//! - provenance metadata that the response will echo back unchanged.
//!
//! The request is constructed by the caller (e.g. the SemanticMiner)
//! and consumed by the LlmPort. The port MUST echo back the request's
//! `provenance` in the response (the lineage contract).

use serde::{Deserialize, Serialize};

use crate::domain::ai::frame::InvestigationFrameId;

/// A bounded tool/read declaration the agent may consult.
///
/// The agent's response (`ToolRead`) carries the same `name` so the
/// lineage layer can pair declarations with actual reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Stable name (e.g. "read_fact", "list_architecture_constraints").
    pub name: String,
    /// Bounded argument list. Empty if the tool takes no arguments.
    pub args: Vec<String>,
}

impl ToolCall {
    /// Construct a tool call. Trims whitespace and rejects empty names.
    pub fn new(
        name: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, &'static str> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err("tool call name must not be empty");
        }
        let args = args.into_iter().map(Into::into).collect();
        Ok(Self { name, args })
    }
}

/// Provenance metadata the caller attaches to a request.
///
/// The port MUST echo this verbatim into the response. A mismatch is
/// treated as a port contract violation by the lineage layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestProvenance {
    /// Frame this request belongs to.
    pub frame_id: InvestigationFrameId,
    /// Free-form caller label (e.g. "semantic-miner-v1").
    pub caller: String,
    /// Wall-clock timestamp supplied by the caller. `None` if the
    /// caller has no clock.
    pub issued_at: Option<String>,
}

impl RequestProvenance {
    /// Construct, rejecting empty caller.
    pub fn new(
        frame_id: InvestigationFrameId,
        caller: impl Into<String>,
        issued_at: Option<String>,
    ) -> Result<Self, &'static str> {
        let caller = caller.into();
        if caller.trim().is_empty() {
            return Err("caller label must not be empty");
        }
        Ok(Self {
            frame_id,
            caller,
            issued_at,
        })
    }
}

/// The complete input to one LlmPort invocation.
///
/// The frame is the bound; the `instruction` is the bounded textual
/// question. The `tools` are the declared tool/read surface; the
/// agent may NOT call a tool that was not declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationRequest {
    frame_id: InvestigationFrameId,
    /// The frame this request was derived from. Stored by id only —
    /// the request must not carry a deep clone of the frame.
    instruction: String,
    tools: Vec<ToolCall>,
    provenance: RequestProvenance,
}

impl InvestigationRequest {
    /// Construct a request. The `instruction` is bounded by the
    /// frame's `max_response_bytes` only conceptually; the port
    /// enforces its own limit.
    pub fn try_new(
        frame_id: InvestigationFrameId,
        instruction: impl Into<String>,
        tools: Vec<ToolCall>,
        provenance: RequestProvenance,
    ) -> Result<Self, &'static str> {
        let instruction = instruction.into();
        if instruction.trim().is_empty() {
            return Err("instruction must not be empty");
        }
        Ok(Self {
            frame_id,
            instruction,
            tools,
            provenance,
        })
    }

    /// The frame id this request was derived from.
    pub fn frame_id(&self) -> InvestigationFrameId {
        self.frame_id
    }

    /// The bounded textual instruction.
    pub fn instruction(&self) -> &str {
        &self.instruction
    }

    /// The declared tool/read surface.
    pub fn tools(&self) -> &[ToolCall] {
        &self.tools
    }

    /// The provenance metadata (echoed by the port).
    pub fn provenance(&self) -> &RequestProvenance {
        &self.provenance
    }

    /// Stable content digest over the request. The port records this
    /// digest in the response so the lineage trail can pair them.
    pub fn content_digest(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;
        let mix = |acc: &mut u64, bytes: &[u8]| {
            for b in bytes {
                *acc ^= *b as u64;
                *acc = acc.wrapping_mul(prime);
            }
        };
        mix(&mut h, &self.frame_id.get().to_le_bytes());
        mix(&mut h, self.instruction.as_bytes());
        for t in &self.tools {
            mix(&mut h, t.name.as_bytes());
            for a in &t.args {
                mix(&mut h, a.as_bytes());
            }
        }
        mix(&mut h, self.provenance.caller.as_bytes());
        if let Some(ts) = &self.provenance.issued_at {
            mix(&mut h, ts.as_bytes());
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ai::InvestigationFrameId;

    #[test]
    fn tool_call_rejects_empty_name() {
        assert!(ToolCall::new("", Vec::<String>::new()).is_err());
        assert!(ToolCall::new("   ", Vec::<String>::new()).is_err());
    }

    #[test]
    fn request_rejects_empty_instruction() {
        let prov =
            RequestProvenance::new(InvestigationFrameId::from_content_digest(1), "test", None)
                .unwrap();
        let err = InvestigationRequest::try_new(
            InvestigationFrameId::from_content_digest(1),
            "   ",
            vec![],
            prov,
        )
        .unwrap_err();
        assert!(err.contains("instruction"));
    }

    #[test]
    fn the_request_content_digest_is_stable() {
        let id = InvestigationFrameId::from_content_digest(7);
        let prov = RequestProvenance::new(id, "test", Some("2026-09-17T00:00:00Z".into())).unwrap();
        let r1 = InvestigationRequest::try_new(id, "q", vec![], prov.clone()).unwrap();
        let r2 = InvestigationRequest::try_new(id, "q", vec![], prov).unwrap();
        assert_eq!(r1.content_digest(), r2.content_digest());
    }

    #[test]
    fn the_request_provenance_round_trips() {
        let id = InvestigationFrameId::from_content_digest(1);
        let prov = RequestProvenance::new(id, "miner", None).unwrap();
        assert_eq!(prov.frame_id, id);
        assert_eq!(prov.caller, "miner");
    }
}
