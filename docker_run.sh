#!/bin/sh
# The idea is, if the storage folder holds an image folder
# we have likely already run, another file would be fine,
# I just find that a bit messy :P (though this is stupid)
if [ ! -f "/storage/img" ]; then
	watame --action create-folders
	watame --action install-schema
	watame --action clear-sessions
fi

exec watame
