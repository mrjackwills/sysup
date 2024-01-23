#!/bin/bash

case "$(arch)" in
x86_64) SUFFIX="x86_64" ;;
aarch64) SUFFIX="aarch64" ;;
armv6l) SUFFIX="armv6" ;;
esac

if [ -n "$SUFFIX" ]; then
	SYSUP_GZ="sysup_linux_${SUFFIX}.tar.gz"
	wget "https://github.com/mrjackwills/sysup/releases/latest/download/${SYSUP_GZ}"
	tar xzvf "${SYSUP_GZ}" sysup
	rm "${SYSUP_GZ}"
fi
