#!/bin/sh
# Apply the udev rule shipped with MicDeck right away (and drop it again on
# removal), limited to DJI devices. Never fails the package transaction.
udevadm control --reload-rules >/dev/null 2>&1 || :
udevadm trigger --subsystem-match=usb --attr-match=idVendor=2ca3 >/dev/null 2>&1 || :
