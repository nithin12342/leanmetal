# Global Instructions

| Tier | Size | Review & Cognitive Load | Scope & Application |
| :--- | :--- | :--- | :--- |
| **Micro / Surgical** | **1 – 10 lines** | Zero cognitive load. Spot checks take under 30 seconds. | Typo fixes, off-by-one errors, env flags, single config tweaks. Can be cherry-picked with near-zero conflict risk. |

## Instructions for Micro / Surgical Changes
- **Strict Size Limit**: Keep diffs strictly within 1 – 10 lines with zero collateral edits.
- **Fast Spot-Check**: Ensure changes are self-evident and reviewable in under 30 seconds.
- **Permitted Scope**: Limit to typo fixes, boundary/off-by-one corrections, env flags, and isolated config tweaks.
- **Isolation**: Ensure zero merge conflict risk and clean cherry-pickability across branches.

## Remote Push Schedule Constraint
- **Restricted Window**: Never push to remote between 09:00 and 18:00 (9 AM – 6 PM) from Monday to Friday.
- **Permitted Window**: Push only outside 09:00 – 18:00 on weekdays, or anytime on weekends.
