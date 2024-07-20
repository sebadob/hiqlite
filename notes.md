# Backups

- [x] create backup logic for sql writer
- [ ] add data structures to be able to forward a backup task all the way from the `DbClient`
    - [x] state machine impl -> sql writer
    - [ ] `DbClient` -> client stream -> server stream -> state machine impl
- [ ] backups should be possible with file and / or s3 target
- [ ] backups names should incl node id and timestamp, and maybe last applied log id
  to make a restore later on easier (needs a safe parser for the file name)
- [ ] start sqlite backup with `VACUUM INTO` just like for snapshots
- [ ] if an s3 bucket is given, encrypt and push to s3 with `cryptr`

-> We don't need to back up the logs. This would only be important to feature PITR, which is not a goal currently.
When we don't backup logs, we also don't need to care about folder compression and so on, because we would have
a single file only. This make the whole process a lot simpler and less error prone.

## Restore

When applying a backup, we need to cleanup the `_metadata` from raft logs and cleanup the whole raft logs
(during an online backup at least) to make sure we don't panic. This would basically reset the logs counter, which
is a beneficial side-effect for very long running databases under heavy load (probably 10+ years ?).

We will not cleanup `_metadata` ahead of time to still have the possibility to take a look at backups manually.

## Housekeeping

- [ ] implement a backup cleanup policy for both local file and remote s3
- [ ] spawn the backup cleanup task when starting the raft node
