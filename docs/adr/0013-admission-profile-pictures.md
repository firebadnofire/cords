# ADR 0013: Signed profile pictures in admission requests

Status: Accepted under the 2026-10-21 request to include signed PFPs before approval.

This supersedes AGENTS.md section 9.2's restriction of pending request presentation
to public identity and requested name, and ADR 0005's name-only admission card.
It does not change account/device authority, admission, channel or MLS permissions.

Servers advertise `admission-profile-v1` in signed discovery. Compatible clients
may include `admission_profile` in the device-possession session request for the
`join` purpose. It carries the existing canonical signed profile records and the
agreed JPEG XL picture, with no URLs, private preview substitution or new signing
domain. Sharing a profile with staff is disclosed before requesting admission.

History must be complete, account-bound and free of unresolved conflicts. The
request snapshot supports up to 256 records / 1 MiB of serialized history and the
existing 5,000,000-byte image limit. An 8 MiB request limit covers Base64 and
metadata. Unsupported history/media causes an explicit failure; never compact
history or fabricate a new genesis to fit. Image hash, dimensions and codec limits
are validated using the existing sandbox. Device possession precedes validation;
current author authorization, revocation and burn checks precede storage.

Store this optional snapshot with the pending request, without publishing it into
the admitted account directory or issuing any session. Updates preserve previously
observed history. Queue pages expose only availability; a separate authenticated
`GET /api/v1/membership-requests/{device}/profile` requires current server-management
authority. Burned identities and approved/removed requests expose no snapshot.
Approval clears the snapshot; rejected requests retain it while retained for review.
Regular signed profile publication continues independently after admission.

Reviewing clients independently verify signatures, ancestry, account binding and
image digest. Merge history with the selected vault's verified cache so replayed
requests cannot select an older profile. Reuse native avatar rendering and the
existing bounded media cache. Missing pictures use initials; fetch/validation
failures must be visible and retryable. Account/server switches clear presentation.

Older clients and servers remain compatible because the upload field is optional,
omitted when absent, and sent only following signed feature negotiation. Queue
reads opt in with `profiles=true`; availability is omitted from older clients'
responses. Old requests have
no picture until the requester submits a fresh proof with a profile snapshot.

Security tradeoff: server operators and authorized admission reviewers receive the
applicant's public signed profile and image before admission. This includes signed
presentation ancestry needed for verification. Private keys, encrypted history and
channel access are never supplied by this flow.
