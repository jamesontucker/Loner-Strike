# ls-control

Loner-Strike's shared control service for servers on a Linux machine (source:
`app/server-agent`). The app puts it on the server over SSH, runs it as
`lonerstrike-<id>-control.service` and checks it against `ls-control.sha256`.

Both files here are built, not kept in git: `./scripts/build-server-agent.sh` (Docker),
and the release workflow builds them for every release (job `server-agent`).
