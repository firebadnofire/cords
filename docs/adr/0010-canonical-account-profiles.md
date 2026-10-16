# ADR 0010: Canonical account profiles and shared presentation media

- Status: Accepted requirements; runtime activation gated by independent-client acceptance
- Date: 2026-10-15
- Supersedes ADR 0009's device-scoped public-profile ordering, not its media fidelity,
  resource, packaging, or identity-separation requirements.

## Account profile authority

One portable account has one canonical public profile: display name and optional
JPEG XL avatar. A currently authorized device signs routine profile changes bound
to the account and its own authorization chain. Root private keys are not used for
ordinary edits. Device identity, server roles, and account presentation remain
separate. No primary device, first-admitted device, timestamp, arrival order, or
server-local counter decides the public profile.

Revisions form an authenticated causal graph. Each binds the account, authorizing
device, parent hashes, generation, explicit changed fields, and resulting profile.
Generation is zero at genesis and one greater than the maximum parent generation;
it is not a last-writer-wins selector. Signing domains, canonical encoding, bounds,
compatibility negotiation, and fixed vectors must pass before publication activates.
Missing ancestry is pending; an invalid signature or substituted account supplies
no authority. Identical records are idempotent. Persist history and all known heads
across restarts and independently hosted servers; do not assume federation or
claim knowledge of updates withheld by a server.

Independent field edits merge without user intervention, but the resulting merge
must be signed by a currently authorized device. Causally maximal writes to the
same field with different values are conflicts. Equal concurrent values agree.
Display name and the complete avatar (including shape/crop) are atomic fields.
Keep the last agreed profile visible while conflicting changes are reviewed.
Any authorized device can resolve them with a signed revision referring to every
known resolved head. A later discovered competing branch reopens review. Servers
may distribute records; they cannot sign or fabricate account-authored merges.

Revocation prevents future publication. Previously accepted versions may remain
historical evidence; a claimed old timestamp cannot authorize a new update from
a revoked device. Apply existing identity burn and authorization rules first.
Preserve the signed history required for validation. Initial implementation has
no history compaction; deduplicate encoded images without discarding signed data.

## Migration

Valid legacy device cards are candidates, not competing account profiles. A current
authorized device may explicitly sign a genesis from identical verified candidates.
Different candidate values require account-level review. Invalid/revoked evidence
cannot become authority; no migration invents signatures. Do not expose device
profiles as a canonical account directory while this gate is incomplete.

## Shared media and independent ownership

Reuse ADR 0009's pinned libjxl/source WASM, incremental frame processing, exact
timing, lossless alpha, worker watchdogs, and 512 MiB aggregate budget. Profile
avatars retain the 5,000,000-byte JXL limit and signed-profile publication. Server
icons and banners use authenticated server presentation APIs and separate durable
asset storage, not encrypted attachments or account-profile signatures.

Server icon limits: 5,000,000 encoded bytes, 1024 square output. Server banner
limits: 10,000,000 encoded bytes, 1536 by 512 output, 3:1 crop. Both share existing
source/frame/pixel/metadata/deadline limits; output policies cannot increase avatar
limits. All frames use the same crop. SVG is not accepted on these upload paths.
Server name/description/images do not alter hostname, origin, or signing identity.

Bundled canvas playback is required on supported WebViews. Visible imagery plays
by default; pause offscreen/hidden and for OS/application reduced motion or the
application animation-disable preference. Expanded views offer explicit playback.
Do not activate either media workflow from compilation or codec tests alone.

## Acceptance

### Version 1 wire and persistence implementation

The new domain is `CORDS-ACCOUNT-PROFILE-REVISION-V1`. Its deterministic CBOR
statement contains version, account ID, device ID, authorization-record hash,
sorted unique parent revision hashes, causal generation, sorted changed fields,
and complete resulting profile content. Revision identifiers hash this domain's
signing bytes (not a JSON spelling or signature bytes). The full signed device
authorization travels alongside each revision; admission, revocation and identity
burn checks remain independent and must precede new publication.

Image bytes are stored separately by SHA-256, bounded to 5,000,000 bytes of verified
JPEG XL. The signed atomic avatar field contains hash, encoded size, dimensions,
and circle/square shape. Crops are baked identically into every frame, so no
unsigned remote crop or external URL can change the resulting picture. New
profile endpoints require explicit `account-profile-history-v1` discovery support;
existing UserCard signatures are never reinterpreted as account profile revisions.

History reads are paginated metadata; image reads are separate authenticated,
bounded requests. Known records and heads are sealed in the account vault and
shared across that account's joined servers. Database history remains append-only.
No timestamp, device ordering or generation tie-break selects presentation.

Validate multiple authorized devices, two independent servers, concurrent/offline
updates, automatic signed merges, conflict resolution, missing ancestry, replay,
revocation and restart persistence. Validate upload/crop/publication/retrieval and
animated rendering using real clients. Packaged Windows/Linux/macOS playback and
notification evidence is separate from source, mocks, or browser-only evidence.
