# createaddition (Horizon port)

A Horizon port of [createaddition](mrh0/createaddition) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- `createaddition:connector`** (small) and **`createaddition:large_connector`**
- — a block entity whose window is `min(maxIn, maxOut)` = 1,000 FE (small) or
- ,000 FE (large), declared through `block_entity_energy` (API 142) as
- `Capabilities.Energy.BLOCK`. Placed against a face, `facing` points at the
- block it is mounted on (API 76).
- Placement mode**, upstream's `ConnectorMode.test` on the first server tick:
- the mounted block can give → **Pull**, it can only take → **Push**, else
- None**. Stored in the compound (`mode`) and shown in the block's `mode`

## Not yet ported

- The rotation half**: `electric_motor`, `alternator`, `rolling_mill`,
- `tesla_coil`, `servo_motor`, and the `charging/` recipe type (40+ oxidisation
- and charging recipes). Blocked by the missing mod-to-mod API: a rotation
- consumer** could read a neighbour's published `Speed`/`Capacity`/`Stress`
- with API 18, but nothing here needs it, and producing or joining Create's
- network is impossible without forking its engine.
- `small_light_connector`** (its 1 FE/t lamp consumption).
- `creative_energy`**, the e2e's source of last resort: the harness seeds an

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: mrh0/createaddition. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
