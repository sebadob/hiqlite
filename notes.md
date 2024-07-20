# Backups

- [x] the sql writer needs a way to hook in the communication with the logs task
- [ ] add data structures to be able to forward a backup task all the way from the `DbClient`
- [ ] backups should be possible with file and / or s3 target
- [ ] backups should have their own folder incl node id and timestamp, and maybe last applied log id
  to make a restore later on easier (needs a safe parser for the file name)
- [ ] when writer receives a backup request, first create a backup of the logs to not loose some
  if maybe purge or truncate run in between
- [ ] when log backups are completed, start sqlite backup with `VACUUM INTO` just like for snapshots
- [ ] if an s3 bucket is given, compress the whole folder as a .tar.gz, then encrypt and push to s3 with `cryptr`

## Restore

TODO find a way to restore backups in 2 ways:

- just apply a full backup with logs + state machine -> self-explanatory
- have the option to apply a backup from just a sqlite snapshot without logs to make migrations from existing
  sqlite databases possible in an easy way
- restore without logs could also be used for log id resets if ever necessary (probably never with a u64)

## Housekeeping

- [ ] implement a backup cleanup policy for both local file and remote s3
- [ ] spawn the backup cleanup task when starting the raft node
