# Preflight OSS Local Repo Vetting Design

## Goal

Add a new `preflight` capability to `vorker-2` that lets a user paste a public GitHub repository URL or local repository path and have Vorker:

- inspect the repository before execution
- decide whether it is safe enough to attempt
- run it inside an isolated local sandbox
- repair setup and bootstrap issues when reasonable
- verify the deepest trustworthy outcome it can reach
- return a structured report, artifacts, and an optional temporary preview

This feature is local-first. It runs on the user's own machine and is designed as a new `vorker-2` capability, not as a separate hosted product.

## Why This Exists

Developers regularly want to answer a simple question:

"If I paste this repo, can I trust it enough to try, and can I get it running without manually doing all the setup?"

Today that usually means:

- cloning unknown code locally
- reading incomplete docs
- installing random dependencies
- discovering missing env vars and broken scripts
- cleaning up the mess by hand

`Preflight OSS` turns that into a supervised, reproducible flow:

- static risk analysis first
- isolated execution second
- repair and verification only within clear boundaries
- transparent output and artifacts at the end

The product is not "AI judges whether the app is good." The product is:

"Vorker tells you how far this repo gets, what it needed, what it changed, and whether the result is trustworthy."

## Non-Goals

V1 does not try to:

- guarantee support for every repo type
- execute untrusted code directly on the host
- fix product logic or application behavior
- bypass auth, payment, licensing, or external integrations
- inject real secrets
- create forks or pull requests automatically
- support GPU-heavy workloads
- support native desktop apps or mobile apps
- support complex distributed stacks with many moving services

The feature accepts any repo, but it does not promise equal success across repo classes.

## User-Facing Promise

The user experience should remain simple:

1. paste a repo
2. Vorker inspects it
3. Vorker attempts the safest meaningful execution path
4. Vorker reports what happened

The output should always land in one of these states:

- `Static only`
- `Buildable`
- `Runnable`
- `Verified`

Those states are the core contract of the feature.

## Supported Inputs

V1 accepts:

- public GitHub repository URLs
- local repository paths

V1 does not accept:

- private GitHub repositories that require Vorker-managed credentials
- arbitrary ZIP uploads
- repositories that are not Git working trees when passed as local paths

## Outcome Model

### `Static only`

The repository was cloned or opened and analyzed, but Vorker did not execute it.

Reasons include:

- risk too high
- repo class unsupported
- required dependencies unavailable
- execution denied by policy or user approval

### `Buildable`

Vorker successfully installed dependencies or ran the repository's build/test/bootstrap process, but did not reach a meaningful running state.

### `Runnable`

Vorker started the application, CLI, or service successfully, but could not complete the verification step strongly enough to label it verified.

### `Verified`

Vorker reached a repo-class-specific verification target and can justify that result with artifacts.

## Repo Classification

The system accepts any repo, but classifies it before execution so the rest of the pipeline can choose an appropriate strategy.

Initial classes:

- web app
- CLI tool
- library/package
- service/API
- data/tooling repo
- unknown

Signals come from:

- `package.json`
- `requirements.txt`
- `pyproject.toml`
- `Cargo.toml`
- `Dockerfile`
- `docker-compose.yml`
- `Makefile`
- README commands
- common framework/config files

Classification is probabilistic, not absolute. The report should show:

- detected class
- confidence
- chosen strategy

## Verification Rules By Repo Class

### Web app

`Verified` means:

- install or build succeeded
- process started
- app binds to an expected local port
- HTTP health check or simple page load succeeds

Optional browser smoke test can improve confidence but is not required in the first pass if HTTP verification is strong.

### CLI tool

`Verified` means:

- install or build succeeded
- help command, version command, or documented sample command runs successfully

### Library/package

`Verified` means:

- install or build succeeded
- test suite passes if present, or a documented verification command passes

### Service/API

`Verified` means:

- process starts
- health endpoint or equivalent request succeeds if one can be inferred

### Unknown

Unknown repos can still reach `Buildable` or `Runnable`, but should only reach `Verified` when the verification path is explicit and defensible.

## V1 Safety Model

This feature must not be designed around a blacklist alone. Safety is a layered system.

Primary safety layers:

1. isolated sandbox
2. host mount restrictions
3. resource limits
4. network policy
5. command policy
6. user approvals

### Sandbox

V1 uses a hardened local container runtime such as Docker or Podman behind a sandbox interface.

Rationale:

- realistic to ship on user machines
- much easier than jumping directly to microVMs
- still good enough for strong isolation if mounts and networking are controlled tightly

The sandbox must:

- use an ephemeral workspace
- avoid host home directory mounts
- avoid Docker socket passthrough
- avoid access to SSH keys, git credentials, browser state, and shell history
- run with CPU, memory, disk, and time limits

### Filesystem policy

The repository should be copied or cloned into an ephemeral work directory.

Allowed persistent outputs:

- run report
- logs
- patch diff
- metadata artifacts

Everything else should be disposable.

### Network policy

The network model should be phase-based:

- static analysis phase: no execution network needed
- install phase: allow registry/package-manager traffic
- runtime phase: restrict aggressively, default deny except for local verification or explicit allow rules

This is important because many repos need egress to install dependencies, but that does not mean they should keep full outbound access while running.

### Command policy

Command policy exists, but only as a secondary guard.

Examples of disallowed behavior:

- `sudo`
- destructive host filesystem paths
- privilege escalation
- shell patterns that attempt to escape the sandbox
- commands targeting mounted host-sensitive directories

The system should rely on the sandbox first, not on trying to enumerate every dangerous shell command.

## Static Risk Analysis

Before execution, Vorker should produce a risk assessment.

Checks include:

- suspicious install scripts
- `curl | bash` or similar remote execution patterns
- postinstall hooks
- embedded secrets
- obviously destructive shell patterns
- unusual network behavior hints
- stale or unhealthy repo signals when cheaply available

Risk levels:

- low
- medium
- high

Policy:

- low risk: may proceed automatically
- medium risk: warn clearly and allow execution
- high risk: require explicit approval before any execution attempt

The report must show why the repo received its score.

## Repair Policy

The repair boundary must stay narrow or the system becomes untrustworthy.

Allowed changes:

- dependency versions
- install/build/start scripts
- config files
- safe default env template generation
- port bindings
- missing bootstrap steps
- documentation-derived command normalization

Disallowed changes:

- app feature logic
- business rules
- auth semantics
- payment flows
- product decisions
- silent disabling of security-critical behavior

The report should call out when a suggested fix would cross the repair boundary, and stop rather than making the edit.

## Environment Variable Policy

V1 may:

- read `.env.example`, sample config files, and README docs
- generate stub env files with clearly fake placeholder values
- mark unresolved required secrets in the report

V1 may not:

- inject real user secrets automatically
- guess production credentials
- silently fake external integrations and claim the app is verified if they are still required

If a repo cannot be meaningfully verified without real secrets, Vorker should degrade to `Runnable` or `Buildable` and explain why.

## Agent Pipeline

This feature should use a structured pipeline, not a generic swarm.

### 1. Intake Agent

Responsibilities:

- normalize repo input
- clone or open repo
- classify repo type
- build initial strategy

### 2. Risk Agent

Responsibilities:

- static scan
- risk score
- execution recommendation

### 3. Setup Agent

Responsibilities:

- infer runtime and package manager
- install dependencies
- generate safe env defaults if allowed
- prepare bootstrap commands

### 4. Run Agent

Responsibilities:

- start the app, CLI, or service
- capture logs and ports
- identify whether execution reached a stable state

### 5. Debug Agent

Responsibilities:

- parse failures
- identify likely root cause
- decide whether the issue is within repair policy

### 6. Patch Agent

Responsibilities:

- make minimal allowed changes
- emit structured patch reasoning
- produce a diff artifact

### 7. Verify Agent

Responsibilities:

- re-run the flow after patching
- perform repo-class verification
- assign final outcome state

### 8. Report Agent

Responsibilities:

- summarize results
- explain risks
- explain fixes made
- explain remaining blockers

## Repair Loop Rules

The repair loop should be bounded.

V1 defaults:

- maximum wall time: 10 minutes
- maximum repair attempts: 5

The loop stops when:

- `Verified`
- repair boundary would be crossed
- confidence becomes too low
- time budget expires
- risk requires human approval and approval is denied

## Vorker-2 Integration

This feature should appear as a first-class runtime capability.

### CLI

Add:

- `vorker preflight <repo>`

Optional flags can come later, but V1 should not require many upfront knobs.

### Supervisor model

Introduce a new run type or mode representing a preflight execution.

The supervisor needs to track:

- repo source
- classification
- risk state
- sandbox state
- current pipeline stage
- artifacts
- final outcome

### Event model

Add explicit events such as:

- `preflight.created`
- `preflight.classified`
- `preflight.risk_scored`
- `preflight.execution.started`
- `preflight.execution.failed`
- `preflight.patch.generated`
- `preflight.verified`
- `preflight.completed`

This keeps the feature compatible with the existing event-driven supervisor model.

### TUI and web control plane

The operator surfaces should show:

- current preflight stage
- repo class
- risk level
- active sandbox state
- latest failure
- final outcome
- links to logs, diff, and preview

The TUI does not need a special new interaction model for V1. It only needs to render preflight runs coherently as another supervised workflow.

## Artifacts

Artifacts should live under:

- `.vorker-2/preflight/<run-id>/`

Suggested files:

- `report.json`
- `summary.md`
- `risk.json`
- `strategy.json`
- `logs/`
- `patch.diff`
- `metadata.json`

This artifact directory is the reproducibility contract of the feature.

## Preview Model

When a web app reaches `Runnable` or `Verified`, Vorker may expose a local preview via a controlled local proxy.

Requirements:

- preview is local-only by default
- no public sharing by default
- preview URL appears in the report and operator surfaces

This is useful, but preview is an output of success, not the primary feature contract.

## Reporting Requirements

The final report must answer:

- what repo was analyzed
- what class it was identified as
- what risks were found
- what strategy was attempted
- what commands were run
- what changed
- what final state was reached
- why it stopped there
- what the user should do next

The report is not optional. Trust depends on it.

## V1 Success Criteria

V1 is successful when all of the following are true:

1. `vorker preflight <repo>` exists and runs locally.
2. The feature accepts both public GitHub URLs and local repo paths.
3. Static risk analysis runs before any execution.
4. Execution happens only inside the sandbox implementation.
5. The system can reach `Static only`, `Buildable`, `Runnable`, or `Verified` and explain why.
6. Allowed fixes are emitted as a patch diff artifact.
7. Reports and logs are written under `.vorker-2/preflight/<run-id>/`.
8. The supervisor can surface progress and final state to the TUI and web UI.

## Explicit V1 Tradeoff

V1 should optimize for:

- honest best-effort on any repo

and not for:

- maximal automation at any cost

If the system is uncertain, it should downgrade confidence and explain itself rather than forcing a fake `Verified` result.

## Future Extensions

These are valid later directions, but not part of V1:

- private repo auth flows
- GitHub App integration
- automatic fork and PR generation
- deeper browser testing
- remote or cloud sandbox pools
- multi-service orchestration
- stronger OS-level sandbox backends beyond containers

## Bottom Line

`Preflight OSS` fits `vorker-2` if it is treated as:

- a local supervised repo-vetting workflow
- backed by strong isolation
- bounded by a narrow repair policy
- honest about support depth

The right product promise is:

"Paste a repo, and Vorker will tell you how far it can safely and reproducibly get."

That is concrete enough to implement and broad enough to be useful.
