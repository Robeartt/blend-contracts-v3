# Pool hooks

A hook is a contract a pool calls once after every submit that contains an entry into the
pool (supply, borrow, or an auction fill) and that its backstop calls after every deposit. The
hook sees the batch, the touched reserves and the user's final positions, and it can reject
the batch by panicking. It never moves funds and it is never called on exits: repay and
withdraw from the pool, and `queue_withdrawal`, `dequeue_withdrawal` and `withdraw` from the
backstop, always go through, so a hook can gate who enters a pool but cannot trap anyone in it.

The hook address is set when the factory deploys the pool and cannot be changed. The UI shows
it next to the pool's oracle and admin, linked to stellar.expert.

## Interface

```rust
fn on_submit(e: Env, from: Address, spender: Address, to: Address,
             requests: Vec<Request>, reserves: Vec<Reserve>, positions: Positions);
fn on_backstop_deposit(e: Env, from: Address, amount: i128, shares: i128);
```

Both functions must exist. A hook that has no rule for the backstop implements
`on_backstop_deposit` as a no-op, as `gated-liquidator` and `supplier-only` do. The types come
from the pool's WASM spec, imported with `contractimport!`; do not depend on the `pool` crate
itself, that would export the pool's entry points into your hook.

## Reference hooks

| crate | rule |
| --- | --- |
| `allowlist` | every party to a pool entry and every backstop depositor must be on the owner's list |

| `gated-liquidator` | only listed addresses may fill auctions; everything else passes |
| `supplier-only` | `Supply` only from one address (the operator's fee vault); collateral, borrowing and the backstop are open |

Each is an OpenZeppelin `ownable` contract; the owner manages the list with `set_allowed` /
`set_supplier`. Rejections use error code 1500.

## Building a hook so its source is verifiable

Users see your hook as an address. Make that address say who you are: build and release it
with the [stellar-expert soroban-build-workflow](https://github.com/stellar-expert/soroban-build-workflow)
from a tagged commit in a public repository. The workflow produces a reproducible WASM, a
GitHub release with the artifact and a build attestation; stellar.expert then shows the
repository, the tag and the verified source next to the contract's code hash, which is what
the pool's Hook link opens. Set `home_domain` so the explorer can also link your domain.

This repository's [`release.yml`](../.github/workflows/release.yml) releases the three
reference hooks that way on every `v*` tag; copy one of its jobs, point `package` at your
crate and keep the `permissions` block. A hook built by hand and uploaded from a laptop shows
up as an anonymous code hash, and pool users have no reason to trust it.
