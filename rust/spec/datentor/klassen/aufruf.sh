#!/bin/sh
# Wrapper for the outside runner: DT_BIN=klassen/aufruf.sh calls `datentor klassen`.
# DATENTOR names the program; without it, `datentor` comes from PATH.
exec "${DATENTOR:-datentor}" klassen "$@"
