# Triage labels

Label strings equal the canonical role names, so no mapping is needed. All of them already exist in SckraT/OpenSCL4; create one only if it goes missing: gh label create NAME --repo SckraT/OpenSCL4.

## Category roles (exactly one per issue)

| Role | Tracker label | Meaning |
| --- | --- | --- |
| bug | bug | Something is broken |
| enhancement | enhancement | New feature or improvement |

## State roles (exactly one per issue)

| Canonical role | Tracker label | Meaning |
| --- | --- | --- |
| needs-triage | needs-triage | Maintainer needs to evaluate |
| needs-info | needs-info | Waiting on reporter for information |
| ready-for-agent | ready-for-agent | Fully specified, ready for an AFK agent |
| ready-for-human | ready-for-human | Requires human implementation |
| wontfix | wontfix | Will not be actioned |

Every triaged issue carries exactly one category label and one state label. If they conflict, ask the maintainer before changing anything.

The wayfinder:* labels are a separate namespace, not triage roles.
