# dice-bot-rs
A Discord bot for rolling dice.

Add the bot with [this link](https://discordapp.com/oauth2/authorize?client_id=475156037578784779&scope=bot&permissions=64)

## Commands
Prefix `?` or `,` (configurable), or mention the bot:

- `?roll EXPR…` (or `?dice`): roll dice, e.g. `?roll 2d6+3 1d20`. Several space-separated expressions roll separately.
  Bare numbers roll the default die (d20): `?roll` is 1d20, `?roll +2` is 1d20+2, `?roll 3` is three d20s.
- `?ten EXPR…`: the same, but bare numbers roll d10s.
- `?rollsp` / `?tensp`: the same, with the result hidden in a spoiler.
- Shortcuts: `?roll3`, `?roll+2`, `?ten+1`.
- `?help`: this list.

Limits: 50 expressions, 70 terms each, ±5000 dice per term, 4,000,000 sides. Replies that would exceed Discord's
2000-character limit drop the individual dice, then shorten the echoed expression.

## Configuration
Settings come from the environment; see `.env.example`. Secrets live in a dotenvx-encrypted `.env` with its private
key in `.env.keys` next to it. Neither file is committed.

```sh
cp .env.example .env    # fill in DISCORD_TOKEN
dotenvx encrypt         # encrypts .env in place, writes .env.keys
chmod 600 .env .env.keys
```

## Development
```sh
cargo test
cargo clippy --all-targets -- -D warnings
./run.sh                # release build, run with decrypted secrets
```

## Deployment (dcprod)
`deploy/rollbot.service` is the systemd unit, installed as `/etc/systemd/system/rollbot.service`:

```sh
git pull --ff-only && cargo build --release
sudo install -m 644 deploy/rollbot.service /etc/systemd/system/rollbot.service
sudo systemctl daemon-reload && sudo systemctl restart rollbot
journalctl -u rollbot -f    # expect "DCRollBot is connected!"
```
