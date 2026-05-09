"""LLM client for MiniMax M2.7 HighSpeed via Anthropic-compatible API.

Uses environment variables:
  ANTHROPIC_BASE_URL  → https://api.minimax.io/anthropic
  ANTHROPIC_AUTH_TOKEN → JWT Bearer token
  ANTHROPIC_MODEL     → MiniMax-M2.7-highspeed
"""

import os
import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)

# ═══════════════════════════════════════════════════════════════════════
# Model Configuration
# ═══════════════════════════════════════════════════════════════════════

class ModelConfig:
    """Configuration for the MiniMax M2.7 HighSpeed model."""
    
    # Defaults from environment
    MODEL = os.environ.get("ANTHROPIC_MODEL", "MiniMax-M2.7-highspeed")
    SMALL_FAST_MODEL = os.environ.get("ANTHROPIC_SMALL_FAST_MODEL", "MiniMax-M2.7-highspeed")
    BASE_URL = os.environ.get("ANTHROPIC_BASE_URL", "https://api.minimax.io/anthropic")
    AUTH_TOKEN = os.environ.get("ANTHROPIC_AUTH_TOKEN", "")
    API_TIMEOUT_MS = int(os.environ.get("API_TIMEOUT_MS", "300000"))
    
    # Agent-specific model assignments
    ANALYZER_MODEL = MODEL
    IMPROVER_MODEL = MODEL
    EVALUATOR_MODEL = MODEL
    DECIDER_MODEL = MODEL
    
    # Token limits
    MAX_TOKENS = 8192
    TEMPERATURE = 0.3  # Low for code editing precision
    
    @classmethod
    def validate(cls) -> bool:
        """Check that all required config is available."""
        if not cls.AUTH_TOKEN:
            logger.error("ANTHROPIC_AUTH_TOKEN not set in environment")
            return False
        if not cls.BASE_URL:
            logger.error("ANTHROPIC_BASE_URL not set in environment")
            return False
        logger.info(f"Model config OK: {cls.MODEL} @ {cls.BASE_URL}")
        return True
    
    @classmethod
    def as_dict(cls) -> dict:
        return {
            "model": cls.MODEL,
            "base_url": cls.BASE_URL,
            "max_tokens": cls.MAX_TOKENS,
            "temperature": cls.TEMPERATURE,
            "timeout_ms": cls.API_TIMEOUT_MS,
        }


# ═══════════════════════════════════════════════════════════════════════
# LLM Client (Anthropic-compatible)
# ═══════════════════════════════════════════════════════════════════════

class LLMClient:
    """Client for MiniMax M2.7 via Anthropic-compatible Messages API."""
    
    def __init__(self, model: Optional[str] = None):
        self.model = model or ModelConfig.MODEL
        self.config = ModelConfig.as_dict()
        self._client = None
    
    @property
    def client(self):
        """Lazy-load Anthropic client with custom base URL."""
        if self._client is None:
            try:
                from anthropic import Anthropic
                self._client = Anthropic(
                    base_url=ModelConfig.BASE_URL,
                    api_key=ModelConfig.AUTH_TOKEN,
                    timeout=float(ModelConfig.API_TIMEOUT_MS) / 1000.0,
                )
                logger.info(f"Anthropic client initialized: {ModelConfig.BASE_URL}")
            except ImportError:
                logger.error("anthropic package not installed. Run: pip install anthropic")
                raise
        return self._client
    
    def chat(self, system: str, messages: list, 
             max_tokens: int = None, temperature: float = None) -> str:
        """Send a chat completion request.
        
        Args:
            system: System prompt
            messages: List of {"role": "user"|"assistant", "content": "..."} dicts
            max_tokens: Override default max tokens
            temperature: Override default temperature
            
        Returns:
            Response text from the model
        """
        max_tokens = max_tokens or ModelConfig.MAX_TOKENS
        temperature = temperature or ModelConfig.TEMPERATURE
        
        try:
            response = self.client.messages.create(
                model=self.model,
                max_tokens=max_tokens,
                temperature=temperature,
                system=[{"type": "text", "text": system}],
                messages=messages,
            )
            return response.content[0].text
        except Exception as e:
            logger.error(f"LLM call failed: {e}")
            raise
    
    def analyze_rule(self, rule_id: str, rule_code: str, 
                     metrics: dict, language: str = "rust") -> Dict[str, Any]:
        """Use LLM to analyze a rule and suggest improvements.
        
        Returns dict with:
            - analysis: human-readable analysis
            - improvement_type: "regex_tighten" | "pattern_extend" | "threshold_tune" | "refactor" | "none"
            - suggested_changes: list of specific changes
            - confidence: 0.0-1.0
        """
        system = """You are a code quality expert specialized in static analysis rules.
Analyze the given rule and its performance metrics. Suggest specific, actionable improvements.

Focus on:
1. Regex patterns: Can they be tightened to reduce false positives?
2. Detection logic: Are edge cases covered?
3. Thresholds: Are parameters optimal for the current codebase?
4. Clarity: Is the rule easy to understand and maintain?

Return ONLY valid JSON with this schema:
{
  "analysis": "string explaining your reasoning",
  "improvement_type": "regex_tighten" | "pattern_extend" | "threshold_tune" | "refactor" | "none",
  "suggested_changes": ["change 1", "change 2"],
  "confidence": 0.0-1.0
}"""

        f1 = metrics.get("f1", "N/A")
        fpr = metrics.get("fpr", "N/A")
        
        user_message = f"""Rule ID: {rule_id}
Language: {language}
Current Metrics: F1={f1}, FPR={fpr}, Precision={metrics.get('precision', 'N/A')}, Recall={metrics.get('recall', 'N/A')}

Rule Code:
```rust
{rule_code[:3000]}
```

Based on these metrics and the rule code, what specific improvements would you suggest?"""

        try:
            response = self.chat(system=system, messages=[{"role": "user", "content": user_message}])
            
            # Parse JSON from response
            import json
            import re
            
            # Extract JSON block
            json_match = re.search(r'\{[\s\S]*\}', response)
            if json_match:
                return json.loads(json_match.group(0))
            
            logger.warning("Could not parse JSON from LLM response")
            return {
                "analysis": response[:500],
                "improvement_type": "none",
                "suggested_changes": [],
                "confidence": 0.3
            }
        except Exception as e:
            logger.error(f"LLM analysis failed: {e}")
            return {
                "analysis": f"Error: {e}",
                "improvement_type": "none",
                "suggested_changes": [],
                "confidence": 0.0
            }
    
    def generate_improvement_description(self, rule_id: str, 
                                          changes: list, metrics_delta: dict) -> str:
        """Generate a concise git commit message for kept changes."""
        system = "You generate concise git commit messages for static analysis rule improvements."
        
        delta_f1 = metrics_delta.get("delta_f1", 0)
        delta_fpr = metrics_delta.get("delta_fpr", 0)
        
        user = f"""Rule {rule_id} was improved.
Changes: {', '.join(changes)}
Metrics: ΔF1={delta_f1:+.3f}, ΔFPR={delta_fpr:+.3f}

Generate a concise commit message (max 72 chars). Format: 'autoresearch: {rule_id} — ...'"""

        try:
            return self.chat(system=system, messages=[{"role": "user", "content": user}], 
                           max_tokens=100, temperature=0.1).strip()
        except:
            return f"autoresearch: improve {rule_id}"


# ═══════════════════════════════════════════════════════════════════════
# Singleton
# ═══════════════════════════════════════════════════════════════════════

_llm_client: Optional[LLMClient] = None


def get_llm_client() -> LLMClient:
    """Get or create the global LLM client instance."""
    global _llm_client
    if _llm_client is None:
        if not ModelConfig.validate():
            logger.warning("LLM client not available — using heuristic mode")
        _llm_client = LLMClient()
    return _llm_client
