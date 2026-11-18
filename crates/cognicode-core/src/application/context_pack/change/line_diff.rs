//! The exact changed lines between two versions of a text.
//!
//! The SCM kernel answers *which files* changed, by digest. A reader that
//! wants to know *which lines* needs the two byte strings and a diff, and
//! this is that diff: pure, deterministic and independent of any VCS.
//!
//! ## Contract
//!
//! - Lines are split after `\n` and keep their terminator, so a file that
//!   gains or loses its final newline changes its last line, as it does in
//!   `git diff`. Bytes are compared as they are: a change of line endings is
//!   a change of every line.
//! - The result is a **minimal** line diff (shortest edit script). When more
//!   than one minimal diff exists, the placement of a hunk inside a run of
//!   equal lines may differ from the one Git picks; the number of changed
//!   lines never does.
//! - Past [`MAX_EDITS`] edits the diff is not computed: one hunk covers
//!   everything between the common prefix and the common suffix and the
//!   result says it is not exact. A superset of the changed lines is
//!   reported, never a subset.

/// The most edits the minimal diff is searched for before it falls back to
/// one coarse hunk.
pub const MAX_EDITS: usize = 1500;

/// A run of lines of one side of a diff. Line indexes count from zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The index of the first line; for an empty span, the number of lines
    /// that precede the position.
    pub at: u32,
    /// How many lines.
    pub len: u32,
}

/// One change: the lines of the old text replaced by the lines of the new.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    /// The lines of the old text that are gone.
    pub removed: Span,
    /// The lines of the new text that are there instead.
    pub added: Span,
}

impl Hunk {
    /// The lines of the new text this hunk touches, counted from one and
    /// both ends included; `None` for a pure deletion.
    pub fn added_lines(&self) -> Option<(u32, u32)> {
        (self.added.len > 0).then(|| (self.added.at + 1, self.added.at + self.added.len))
    }
}

/// The hunks that turn one text into the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDiff {
    /// The changes, in order, none touching another.
    pub hunks: Vec<Hunk>,
    /// Whether the hunks are the minimal diff; `false` when the edit budget
    /// was spent and they are a superset.
    pub exact: bool,
}

/// The hunks between `before` and `after`.
pub fn line_diff(before: &str, after: &str) -> LineDiff {
    let old: Vec<&str> = before.split_inclusive('\n').collect();
    let new: Vec<&str> = after.split_inclusive('\n').collect();
    let prefix = old
        .iter()
        .zip(&new)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];

    let (hunks, exact) = match edit_marks(old_mid, new_mid) {
        Some((removed, added)) => (hunks_of(&removed, &added), true),
        None => (vec![coarse_hunk(old_mid.len(), new_mid.len())], false),
    };
    LineDiff {
        hunks: hunks
            .into_iter()
            .map(|hunk| shifted(hunk, prefix as u32))
            .collect(),
        exact,
    }
}

/// The hunk that covers the whole middle of both texts.
fn coarse_hunk(removed: usize, added: usize) -> Hunk {
    Hunk {
        removed: Span {
            at: 0,
            len: removed as u32,
        },
        added: Span {
            at: 0,
            len: added as u32,
        },
    }
}

fn shifted(hunk: Hunk, by: u32) -> Hunk {
    Hunk {
        removed: Span {
            at: hunk.removed.at + by,
            len: hunk.removed.len,
        },
        added: Span {
            at: hunk.added.at + by,
            len: hunk.added.len,
        },
    }
}

/// Which lines of the old text are deleted and which of the new are
/// inserted in a shortest edit script, or `None` past [`MAX_EDITS`].
///
/// Myers' O(ND) greedy search: `trace[d]` keeps the furthest-reaching
/// points before round `d`, and the walk back through it names the edits.
fn edit_marks(old: &[&str], new: &[&str]) -> Option<(Vec<bool>, Vec<bool>)> {
    let (n, m) = (old.len() as isize, new.len() as isize);
    let mut removed = vec![false; old.len()];
    let mut added = vec![false; new.len()];
    if n == 0 && m == 0 {
        return Some((removed, added));
    }
    let limit = (n + m).min(MAX_EDITS as isize);
    let offset = limit + 1;
    let at = |k: isize| (k + offset) as usize;
    let mut v = vec![0isize; (2 * limit + 3) as usize];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    let mut reached = None;
    'search: for d in 0..=limit {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d || (k != d && v[at(k - 1)] < v[at(k + 1)]) {
                v[at(k + 1)]
            } else {
                v[at(k - 1)] + 1
            };
            let mut y = x - k;
            while x < n && y < m && old[x as usize] == new[y as usize] {
                x += 1;
                y += 1;
            }
            v[at(k)] = x;
            if x >= n && y >= m {
                reached = Some(d);
                break 'search;
            }
            k += 2;
        }
    }
    let total = reached?;

    let (mut x, mut y) = (n, m);
    for d in (1..=total).rev() {
        let v = &trace[d as usize];
        let k = x - y;
        let previous = if k == -d || (k != d && v[at(k - 1)] < v[at(k + 1)]) {
            k + 1
        } else {
            k - 1
        };
        let (previous_x, previous_y) = (v[at(previous)], v[at(previous)] - previous);
        while x > previous_x && y > previous_y {
            x -= 1;
            y -= 1;
        }
        if x == previous_x {
            added[previous_y as usize] = true;
        } else {
            removed[previous_x as usize] = true;
        }
        x = previous_x;
        y = previous_y;
    }
    Some((removed, added))
}

/// Groups the marks into hunks: a run of deleted and inserted lines with
/// no equal line between them is one hunk.
fn hunks_of(removed: &[bool], added: &[bool]) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < removed.len() || j < added.len() {
        let changing = (i < removed.len() && removed[i]) || (j < added.len() && added[j]);
        if !changing {
            i += 1;
            j += 1;
            continue;
        }
        let (start_i, start_j) = (i, j);
        while (i < removed.len() && removed[i]) || (j < added.len() && added[j]) {
            while i < removed.len() && removed[i] {
                i += 1;
            }
            while j < added.len() && added[j] {
                j += 1;
            }
        }
        hunks.push(Hunk {
            removed: Span {
                at: start_i as u32,
                len: (i - start_i) as u32,
            },
            added: Span {
                at: start_j as u32,
                len: (j - start_j) as u32,
            },
        });
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunk(removed: (u32, u32), added: (u32, u32)) -> Hunk {
        Hunk {
            removed: Span {
                at: removed.0,
                len: removed.1,
            },
            added: Span {
                at: added.0,
                len: added.1,
            },
        }
    }

    #[test]
    fn equal_texts_have_no_hunks() {
        let diff = line_diff("a\nb\n", "a\nb\n");
        assert!(diff.hunks.is_empty());
        assert!(diff.exact);
    }

    #[test]
    fn a_replaced_line_is_one_hunk_on_both_sides() {
        let diff = line_diff("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(diff.hunks, vec![hunk((1, 1), (1, 1))]);
        assert_eq!(diff.hunks[0].added_lines(), Some((2, 2)));
    }

    #[test]
    fn an_inserted_line_has_nothing_removed() {
        let diff = line_diff("a\nc\n", "a\nb\nc\n");
        assert_eq!(diff.hunks, vec![hunk((1, 0), (1, 1))]);
    }

    #[test]
    fn a_deleted_line_is_a_point_in_the_new_text() {
        let diff = line_diff("a\nb\nc\n", "a\nc\n");
        assert_eq!(diff.hunks, vec![hunk((1, 1), (1, 0))]);
        assert_eq!(diff.hunks[0].added_lines(), None);
    }

    #[test]
    fn separate_changes_are_separate_hunks() {
        let diff = line_diff("a\nb\nc\nd\ne\n", "A\nb\nc\nd\nE\n");
        assert_eq!(diff.hunks, vec![hunk((0, 1), (0, 1)), hunk((4, 1), (4, 1))]);
    }

    #[test]
    fn the_final_newline_is_part_of_the_last_line() {
        let diff = line_diff("a\nb", "a\nb\n");
        assert_eq!(diff.hunks, vec![hunk((1, 1), (1, 1))]);
    }

    #[test]
    fn a_new_file_is_all_added_and_a_removed_file_all_removed() {
        assert_eq!(line_diff("", "a\nb\n").hunks, vec![hunk((0, 0), (0, 2))]);
        assert_eq!(line_diff("a\nb\n", "").hunks, vec![hunk((0, 2), (0, 0))]);
    }

    #[test]
    fn a_moved_block_is_removed_and_added_not_lost() {
        let diff = line_diff("a\nb\nc\nd\n", "c\nd\na\nb\n");
        let removed: u32 = diff.hunks.iter().map(|h| h.removed.len).sum();
        let added: u32 = diff.hunks.iter().map(|h| h.added.len).sum();
        assert_eq!((removed, added), (2, 2));
    }

    #[test]
    fn the_hunks_rebuild_the_new_text() {
        let before = "one\ntwo\nthree\nfour\nfive\nsix\n";
        let after = "one\n2\nthree\nfive\nsix\nseven\n";
        let diff = line_diff(before, after);
        let old: Vec<&str> = before.split_inclusive('\n').collect();
        let new: Vec<&str> = after.split_inclusive('\n').collect();
        let mut rebuilt: Vec<&str> = Vec::new();
        let mut cursor = 0usize;
        for hunk in &diff.hunks {
            rebuilt.extend(&old[cursor..hunk.removed.at as usize]);
            let at = hunk.added.at as usize;
            rebuilt.extend(&new[at..at + hunk.added.len as usize]);
            cursor = (hunk.removed.at + hunk.removed.len) as usize;
        }
        rebuilt.extend(&old[cursor..]);
        assert_eq!(rebuilt, new);
    }

    #[test]
    fn too_many_edits_fall_back_to_a_superset_that_says_so() {
        let before: String = (0..MAX_EDITS + 10).map(|n| format!("old {n}\n")).collect();
        let after: String = (0..MAX_EDITS + 10).map(|n| format!("new {n}\n")).collect();
        let keep = "same\n";
        let diff = line_diff(
            &format!("{keep}{before}{keep}"),
            &format!("{keep}{after}{keep}"),
        );
        assert!(!diff.exact);
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].added.at, 1);
        assert_eq!(diff.hunks[0].added.len as usize, MAX_EDITS + 10);
    }
}
