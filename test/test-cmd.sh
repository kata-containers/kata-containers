#! /bin/bash

PROXYADDR=/tmp/proxy.sock
TARGETADDR=/tmp/target.sock

set -e

# start server
./server &
# sleep a bit to let server spin up
sleep 2

# start proxy
../proxy -l ${PROXYADDR} -s "unix://"${TARGETADDR} &

# do test

FILES=$(find /etc -type f 2>/dev/null || true)

# sleep a bit to let proxy spin up
sleep 2

for f in ${FILES}; do
	if [ -r ${f} ]; then
		echo running test with ${f}
		output=$(./client -f ${f})
		result=$(echo ${output}|grep SUCCESS 2>/dev/null || true)
		if [ x"${result}" == "x" ]; then
			echo test failed with ${output}
			exit 1
		fi
	fi
done

set +e

pkill server 2>/dev/null
pkill proxy 2>/dev/null

rm -f ${PROXYADDR} ${TARGETADDR}

echo test SUCCEEDED
