#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# Use fio to gather storate IO metrics.
# The fio configuration can be modified via environment variables.
# This test is only designed to handle a single file and class of job
# in fio. If you require a more complex fio test then you are probably
# better off writing that by hand or creating a new metrics test.

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Items for our local Dockerfile/image creation
TEST_NAME="fio"
IMAGE="local-fio"
DOCKERFILE="${SCRIPT_PATH}/fio_dockerfile/Dockerfile"
CONTAINER_NAME="fio_test"

# How much RAM and how many (v)CPUs do we give the container under test?
CONTAINER_RAM=${CONTAINER_RAM:-2G}
CONTAINER_CPUS=${CONTAINER_CPUS:-1}

# Important paths from the pov of both the host and the guest.
# Some of them are mapped into both via a docker volume mount.
HOST_OUTPUT_DIRNAME="${HOST_OUTPUT_DIRNAME:-${SCRIPT_PATH}/fio_output}"
HOST_INPUT_DIRNAME="${HOST_INPUT_DIRNAME:-${SCRIPT_PATH}/fio_input}"
# Used on the host side if we do a volume mount test.
HOST_TEST_DIRNAME="${HOST_TEST_DIRNAME:-/tmp}"
GUEST_OUTPUT_DIRNAME="/output"
GUEST_INPUT_DIRNAME="/input"

# This is the dir that fio will actually run the test upon in the container.
# By default the tests will run on the container 'root mount'. If you set
# TEST_VOLUME_MOUNT, then we will mount a volume mount over that test directory
# and the tests will happen on a volume mount.
TEST_VOLUME_MOUNT=${TEST_VOLUME_MOUNT:-0}
GUEST_TEST_DIRNAME="/testdir"

# These define which variety of the tests we will run
#  Which of the read/write/rw tests will we run (we will run all
#  that are set to 1).
#
# By default we do direct random read and write tests (not readwrite)
FIO_READTEST=${FIO_READTEST:-1}
FIO_WRITETEST=${FIO_WRITETEST:-1}
FIO_READWRITETEST=${FIO_READWRITETEST:-0}
FIO_DIRECT=${FIO_DIRECT:-1}
FIO_RANDOM=${FIO_RANDOM:-1}

# The blocksizes we will test. We have a separate set for direct or not
# as direct mode can only use blocksize or larger sizes.
FIO_BLOCKSIZES="${FIO_BLOCKSIZES:-128 256 512 1k 2k 4k 8k 16k 32k 64k}"
FIO_DIRECT_BLOCKSIZES="${FIO_DIRECT_BLOCKSIZES:-4k 8k 16k 32k 64k}"

# Other FIO parameters that are tweakable by setting in the environment.
# Values from here are directly injected into the fio jobfiles used for
# running the tests.
FIO_NUMJOBS=${FIO_NUMJOBS:-4}
#  By default run a time base test
FIO_TIMEBASED=${FIO_TIMEBASED:-1}
#  And 60s seems a good balance to get us repeatable numbers in not too long a time.
FIO_RUNTIME=${FIO_RUNTIME:-60}
#  By default we have no ramp (warmup) time.
FIO_RAMPTIME=${FIO_RAMPTIME:-0}
#  Drop the caches in the guest using fio. Note, we need CAP_SYS_ADMIN in the container
#  for this to work.
FIO_INVALIDATE=${FIO_INVALIDATE:-1}
#  Do not use fallocate. Not all the filesystem types we can test (such as 9p) support
#  this - which can then generate errors in the JSON datastream.
FIO_FALLOCATE=${FIO_FALLOCATE:-none}
#  When running 'direct', the file size should not really matter as nothing should be
#  cached.
#  If you are running cached (direct=0), then it is likely this whole file will fit in
#  the buffercache. You can make this filesize larger than the container RAM size to
#  make a cached test mostly miss the cache - but, depending on the runtime and setup,
#  you might just end up hitting the host side buffercache. You could of course make the
#  filesize bigger than the host RAM size... it might take fio some time to create that
#  testfile though.
FIO_FILESIZE=${FIO_FILESIZE:-1G}
FIO_IOENGINE=${FIO_IOENGINE:-libaio}
FIO_IODEPTH=${FIO_IODEPTH:-16}

# Generate the fio jobfiles into the host directory that will then be shared into
# the container to run the actual tests.
#
# We iterate through the combination of linear/random and read/write/rw and generate
# all files - even if the test config will not then use them all. it just makes this
# loop simpler, and the extra file overhead is tiny.
#
# Note - the jobfiles *only* contain test-invariant items - that is, anything that
# changes between each iteration of fio in this test (such as the blocksize and the
# direct setting) is dynamically set on the fio commandline at runtime.
generate_jobfiles() {
	local n
	local t

	for n in "" rand; do
		for t in read write rw; do
			local testtype="${n}$t"
			local filebase="fio-${testtype}"
			local filename="${filebase}.job"
			local destfile="${HOST_INPUT_DIRNAME}/${filename}"

			echo "; Kata metrics auto generated fio job file" > "${destfile}"
			echo "[global]" >> "${destfile}"
			echo "directory=$GUEST_TEST_DIRNAME" >> "${destfile}"
			echo "filename=$filebase" >> "${destfile}"
			echo "rw=$t" >> "${destfile}"
			echo "numjobs=$FIO_NUMJOBS" >> "${destfile}"
			echo "time_based=$FIO_TIMEBASED" >> "${destfile}"
			echo "runtime=$FIO_RUNTIME" >> "${destfile}"
			echo "ramp_time=$FIO_RAMPTIME" >> "${destfile}"
			echo "invalidate=$FIO_INVALIDATE" >> "${destfile}"
			echo "fallocate=$FIO_FALLOCATE" >> "${destfile}"
			echo "[file1]" >> "${destfile}"
			echo "size=$FIO_FILESIZE" >> "${destfile}"
			echo "ioengine=$FIO_IOENGINE" >> "${destfile}"
			echo "iodepth=$FIO_IODEPTH" >> "${destfile}"
		done
	done
}

# Initialise the system, including getting the container up and running and
# disconnected, ready to run the tests via `docker exec`.
init() {
	# Check tools/commands dependencies
	cmds=("docker")

	init_env
	check_cmds "${cmds[@]}"

	# Ensure our docker image is up to date
	check_dockerfiles_images "$IMAGE" "$DOCKERFILE"

	# Ensure we have the local input and output directories created.
	mkdir -p ${HOST_OUTPUT_DIRNAME} || true
	mkdir -p ${HOST_INPUT_DIRNAME} || true

	# We need to set some level of priv enablement to let fio in the
	# container be able to execute its 'invalidate'.
	RUNTIME_EXTRA_ARGS="--cap-add=SYS_ADMIN"

	# And set the CPUs and RAM up...
	RUNTIME_EXTRA_ARGS="${RUNTIME_EXTRA_ARGS} --cpus=${CONTAINER_CPUS}"
	RUNTIME_EXTRA_ARGS="${RUNTIME_EXTRA_ARGS} -m=${CONTAINER_RAM}"

	# If we are in volume test mode then mount a volume over the top of the testdir
	# in the container.
	# Otherwise, the default is to test on the 'root mount'.
	if [ "$TEST_VOLUME_MOUNT" -eq 1 ]; then
		RUNTIME_EXTRA_ARGS="${RUNTIME_EXTRA_ARGS} -v ${HOST_TEST_DIRNAME}:${GUEST_TEST_DIRNAME}"
	fi

	# Go pre-create the fio job files
	generate_jobfiles

	# Run up the work container, in detached state, ready to then issue 'execs'
	# to it. Do the input and output volume mounts now.
	docker run -d --rm --name="${CONTAINER_NAME}" --runtime=$RUNTIME ${RUNTIME_EXTRA_ARGS} -v ${HOST_OUTPUT_DIRNAME}:${GUEST_OUTPUT_DIRNAME} -v ${HOST_INPUT_DIRNAME}:${GUEST_INPUT_DIRNAME} $IMAGE
}

# Drop the host side caches. We may even want to do this if we are in 'direct' mode as
# the fio 'direct' applies to the guest, and the container map/mount from the host to the
# guest might enable host side cacheing (that is an option on QEMU for instance).
dump_host_caches() {
	# Make sure we flush things down
	sync
	# And then drop the caches
	sudo bash -c "echo 3 > /proc/sys/vm/drop_caches"
}


# Generate the metrics JSON output files.
# arg1: the name of the fio generated JSON results file
# The name of that input file dictates the name of the final metrics
# json output file as well.
generate_results() {
	# Set the TEST_NAME to define the json output file
	TEST_NAME=${1%.json}

	metrics_json_init
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"testimage" : "${IMAGE}",
		"container_RAM" : "${CONTAINER_RAM}",
		"container_CPUS" : "${CONTAINER_CPUS}",
		"volume_test" : "${TEST_VOLUME_MOUNT}",
		"readtest" : "${FIO_READTEST}",
		"writetest" : "${FIO_WRITETEST}",
		"readwritetest" : "${FIO_READWRITETEST}",
		"fio_direct" : "${FIO_DIRECT}",
		"fio_random" : "${FIO_RANDOM}",
		"fio_blocksize" : "${FIO_BLOCKSIZE}",
		"fio_numjobs" : "${FIO_NUMJOBS}",
		"fio_timebased" : "${FIO_TIMEBASED}",
		"fio_runtime" : "${FIO_RUNTIME}",
		"fio_invalidate" : "${FIO_INVALIDATE}",
		"fio_filesize" : "${FIO_FILESIZE}",
		"fio_ioengine" : "${FIO_IOENGINE}",
		"fio_iodepth" : "${FIO_IODEPTH}"
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"

	# And store the raw JSON emitted by fio itself.
	metrics_json_start_array
	# Read in the fio generated results
	json="$(cat ${HOST_OUTPUT_DIRNAME}/$1)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Raw"

	metrics_json_save
}

# Run the actual tests. Arguments:
# $1 - Do we set the fio 'direct' parameter
# $2 - Do we do the random access test (rather than linear test)
#
# This function will run all/none of the read/write/rw tests depending on their
# relevant environment settings.
run_test() {
	local dodirect=$1
	local dorandom=$2
	local randprefix=""
	local fioopts="--bs=${FIO_BLOCKSIZE} --output-format=json"
	local filename=""

	[ "$dorandom" -eq 1 ] && randprefix="rand"
	[ "$dodirect" -eq 1 ] && fioopts="${fioopts} --direct=1"

	if [ "${FIO_READTEST}" -eq 1 ]; then
		filebase="fio-${randprefix}read"
		filename="${filebase}.job"
		outputfilename="${filebase}-${FIO_BLOCKSIZE}.json"
		fioopts="${fioopts} --output=${GUEST_OUTPUT_DIRNAME}/$outputfilename"
		dump_host_caches
		echo " ${filebase}-${FIO_BLOCKSIZE}"
		local output=$(docker exec "${CONTAINER_NAME}" fio $fioopts ${GUEST_INPUT_DIRNAME}/$filename)
		generate_results "$outputfilename"
	fi

	if [ "${FIO_WRITETEST}" -eq 1 ]; then
		filebase="fio-${randprefix}write"
		filename="${filebase}.job"
		outputfilename="${filebase}-${FIO_BLOCKSIZE}.json"
		fioopts="${fioopts} --output=${GUEST_OUTPUT_DIRNAME}/$outputfilename"
		dump_host_caches
		echo " ${filebase}-${FIO_BLOCKSIZE}"
		local output=$(docker exec "${CONTAINER_NAME}" fio $fioopts ${GUEST_INPUT_DIRNAME}/$filename)
		generate_results "$outputfilename"
	fi

	if [ "${FIO_READWRITETEST}" -eq 1 ]; then
		filebase="fio-${randprefix}rw"
		filename="${filebase}.job"
		outputfilename="${filebase}-${FIO_BLOCKSIZE}.json"
		fioopts="${fioopts} --output=${GUEST_OUTPUT_DIRNAME}/$outputfilename"
		dump_host_caches
		echo " ${filebase}-${FIO_BLOCKSIZE}"
		local output=$(docker exec "${CONTAINER_NAME}" fio $fioopts ${GUEST_INPUT_DIRNAME}/$filename)
		generate_results "$outputfilename"
	fi
}

main() {
	# Decide which blocksize set we need
	if [ "${FIO_DIRECT}" -eq 1 ] ; then
		local blocksizes="${FIO_DIRECT_BLOCKSIZES}"
	else
		local blocksizes="${FIO_BLOCKSIZES}"
	fi
	
	# run the set of tests for each defined blocksize
	for b in $blocksizes; do
		FIO_BLOCKSIZE=$b
		run_test "${FIO_DIRECT}" "${FIO_RANDOM}"
	done
}

cleanup() {
	docker kill "${CONTAINER_NAME}" || true
	clean_env
}

init
main
cleanup
