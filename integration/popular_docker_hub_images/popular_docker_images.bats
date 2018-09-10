#!/usr/bin/env bats
# *-*- Mode: sh; sh-basic-offset: 8; indent-tabs-mode: nil -*-*
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for the most popular images from docker hub.

source ${BATS_TEST_DIRNAME}/../../lib/common.bash

setup() {
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
	clean_env
}

@test "[insert data] insert data in an aerospike container" {
	docker run --runtime=$RUNTIME -d --name aerospike aerospike/aerospike-server
	status=1
	set +e
	for i in $(seq 1 5); do
		docker run --rm --runtime=$RUNTIME -i aerospike/aerospike-tools aql -h $(docker inspect -f '{{.NetworkSettings.IPAddress}}' aerospike) -c "insert into test.foo (PK, foo) values ('123','any'); select * from test.foo"
		if [ $? == 0 ]; then
			status=0
			break
		fi
		sleep 1
	done
	set -e
	docker rm -f aerospike
	return $status
}

@test "[display text] hello world in an alpine container" {
	docker run --rm --runtime=$RUNTIME -i alpine sh -c "echo 'Hello, World'"
}

@test "[display release] check os version in an alpine container" {
	docker run --rm --runtime=$RUNTIME alpine cat /etc/alpine-release
}

@test "[display version] run an arangodb container" {
	docker run --rm --runtime=$RUNTIME -e ARANGO_ROOT_PASSWORD=secretword -e ARANGO_NO_AUTH=1 -p 8529:8529 arangodb/arangodb foxx-manager --version
}

@test "[display help] search job_spec in the  help option of a bonita container" {
	docker run --rm --runtime=$RUNTIME -i -e "TENANT_LOGIN=testuser" -e "TENANT_PASSWORD=mysecretword" bonita bash -c "help | grep job"
}

@test "[display version] show the os version inside a buildpack deps container" {
	docker run --rm --runtime=$RUNTIME -i buildpack-deps cat /etc/os-release
}

@test "[display version] display the cqlsh version in a cassandra container" {
	docker run --rm --runtime=$RUNTIME -i cassandra cqlsh --version
}

@test "[python operation] run a celery container" {
	docker run --rm --runtime=$RUNTIME -i celery bash -c "python -m timeit \"[i for i in range(1000)]"\"
}

@test "[create file] cat a file in a centos container" {
	docker run --rm --runtime=$RUNTIME -i centos bash -c "echo "Test" > testfile.txt | cat testfile.txt | grep Test"
}

@test "[display version] display options in a chronograf container" {
	docker run --rm --runtime=$RUNTIME -i chronograf --version
}

@test "[find bundle] check os-core-update bundle in a clearlinux container" {
	docker run --rm --runtime=$RUNTIME -i clearlinux sh -c "ls /usr/share/clear/bundles | grep os-core-update"
}

@test "[display hostname] check hostname in a clearlinux container" {
	docker run --rm --runtime=$RUNTIME -i clearlinux sh -c "hostname"
}

@test "[run java] execute a message in a clojure container" {
	docker run --rm --runtime=$RUNTIME -i clojure bash -c "echo -e 'public class CC{public static void main(String[]a){System.out.println(\"KataContainers\");}}' > CC.java && javac CC.java && java CC"
}

@test "[create file] run a couchbase container" {
	docker run --rm --runtime=$RUNTIME -i couchbase sh -c "mkdir /home/test; ls /home | grep test"
}

@test "[run agent] run a consul container" {
	docker run --rm --runtime=$RUNTIME -i consul sh -c "timeout -t 10 consul agent -dev -client 0.0.0.0 | grep 0.0.0.0"
}

@test "[display version] run a crate container" {
	docker run --rm --runtime=$RUNTIME -i -e CRATE_HEAP_SIZE=1g crate timeout -t 10 crate -v
}

@test "[display nameserver] check the resolv.conf in a crux container" {
	docker run --rm --runtime=$RUNTIME -i crux sh -c "cat /etc/resolv.conf | grep nameserver"
}

@test "[display credentials] instance in a django container" {
	docker run --rm --runtime=$RUNTIME -i --user "$(id -u):$(id -g)" django sh -c "django-admin | grep sqlflush"
}

@test "[command options] run an instance in a docker container" {
	docker run --rm --runtime=$RUNTIME -i docker sh -c "docker inspect --help"
}

@test "[display directory] run an instance in an elixir container" {
	docker run --rm --runtime=$RUNTIME -i elixir sh -c "pwd"
}

@test "[start configuration] run an erlang container" {
	docker run --runtime=$RUNTIME -d erlang erl -name test@erlang.local
}

@test "[display time] date in a fedora container" {
	docker run --rm --runtime=$RUNTIME -i fedora sh -c "date"
}

@test "[display version] search python version in a fedora/tools container" {
	docker run --rm --runtime=$RUNTIME -i fedora/tools sh -c "python --version"
}

@test "[command options] find the timestamp help in a gazebo container" {
	docker run --rm --runtime=$RUNTIME -i gazebo gz log --help
}

@test "[gcc file] run a gcc container" {
	docker run --rm --runtime=$RUNTIME -i gcc bash -c "echo -e '#include<stdio.h>\nint main (void)\n{printf(\"Hello\");return 0;}' > demo.c && gcc demo.c -o demo && ./demo"
}

@test "[java file] run an instance in a glassfish container" {
	docker run --rm --runtime=$RUNTIME -i glassfish bash -c "echo 'public class T{public static void main(String[]a){System.out.println(\"Test\");}}' > T.java && javac T.java && java T"
}

@test "[golang file] run golang container" {
	docker run --rm --runtime=$RUNTIME -i golang bash -c "echo -e 'package main\nimport \"fmt\"\nfunc main() { fmt.Println(\"hello world\")}' > hello-world.go && go run hello-world.go && go build hello-world.go"
}

@test "[golang settings] go env in a golang container" {
	docker run --rm --runtime=$RUNTIME -i golang bash -c "go env | grep GOARCH"
}

@test "[memory settings] set memory size in haproxy container" {
	docker run --rm --runtime=$RUNTIME -i haproxy haproxy -m 2 -v
}

@test "[display version] run haskell container" {
	docker run --rm --runtime=$RUNTIME -i haskell cabal --numeric-version
}

@test "[display text] run hello world container" {
	docker run --rm --runtime=$RUNTIME -i hello-world | grep "Hello from Docker"
}

@test "[display text] run hello seattle container" {
	docker run --rm --runtime=$RUNTIME -i hello-seattle | grep "Hello from DockerCon"
}

@test "[run application] start apachectl in a httpd container" {
	if docker run --rm --runtime=$RUNTIME -i httpd apachectl -k start | grep "Unable to open logs"; then false; else true; fi
}

@test "[python application] run python command in a hylang container" {
	docker run --rm --runtime=$RUNTIME -i hylang bash -c "python -m timeit -s \"L=[]; M=range(1000)\" \"for m in M: L.append(m*2)\""
}

@test "[configuration settings] display config information of a influxdb container" {
	docker run --rm --runtime=$RUNTIME -i influxdb influxd config
}

@test "[create directory] start an instance in iojs container" {
	docker run --rm --runtime=$RUNTIME -i iojs sh -c "mkdir /root/test; ls /root | grep test"
}

@test "[configuration settings] set nick in an irssi container" {
	docker run -d --runtime=$RUNTIME irssi irssi -n test
}

@test "[java application] display configuration parameters in a jetty container" {
	docker run --rm --runtime=$RUNTIME -i jetty bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java; ls -l ./HW.java"
}

@test "[display version] run jetty container" {
	docker run --rm --runtime=$RUNTIME -i jetty --version
}

@test "[ruby application] start jruby container" {
	docker run --rm --runtime=$RUNTIME -i jruby bash -c "jruby -e \"puts 'Containers'\""
}

@test "[julia application] display message in a julia container" {
	docker run --rm --runtime=$RUNTIME -i julia bash -c "julia -e 'println(\"this is a test\")'"
}

@test "[display configuration] run kapacitor container" {
	docker run --rm --runtime=$RUNTIME -i kapacitor bash -c "kapacitord config > kapacitor.conf | ls -l kapacitor.conf"
}

@test "[display version] display information kibana container" {
	docker run --rm --runtime=$RUNTIME -i kibana --version
}

@test "[display configuration] check kong configuration file is valid" {
	docker run --rm --runtime=$RUNTIME -i kong sh -c "kong check /etc/kong/kong.conf.default | grep valid"
}

@test "[display kernel] check kernel version in a mageia container" {
	docker run --rm --runtime=$RUNTIME -i mageia sh -c "uname -a | grep Linux"
}

@test "[display configuration] start an instance of a mariadb container" {
	docker run --rm --runtime=$RUNTIME -i -e MYSQL_ROOT_PASSWORD=secretword  mariadb bash -c "cat /etc/mysql/mariadb.cnf | grep character"
}

@test "[java application] check memory maven container" {
	docker run --rm --runtime=$RUNTIME -i maven bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java"
}

@test "[perl application] run memcached container" {
	docker run --rm --runtime=$RUNTIME -i memcached sh -c "perl -e 'print \"memcachedtest\n\"'"
}

@test "[display version] start mongo container" {
	docker run --rm --runtime=$RUNTIME -i mongo --version
}

@test "[display version] start nats server" {
	docker run --rm --runtime=$RUNTIME -i nats --version
}

@test "[display text] create a file in a neo4j container" {
	docker run --rm --runtime=$RUNTIME -i neo4j sh -c "echo "running" > test; cat /var/lib/neo4j/test"
}

@test "[display configuration] configuration file neurodebian" {
	docker run --rm --runtime=$RUNTIME neurodebian cat /etc/apt/sources.list.d/neurodebian.sources.list
}

@test "[display configuration] run nginx container" {
	docker run --rm --runtime=$RUNTIME nginx dpkg-query -l | grep --color=no "libc"
}

@test "[display configuration] search in a node container" {
	docker run --rm --runtime=$RUNTIME -i node node --v8-options
}

@test "[display search] search in a nuxeo container" {
	docker run --rm --runtime=$RUNTIME -i nuxeo apt-cache search python
}

@test "[perl application] run an odoo container" {
	docker run --rm --runtime=$RUNTIME -i odoo bash -c "perl -e 'print "Hello\n"'"
}

@test "[create files] create files in an oraclelinux container" {
	docker run --rm --runtime=$RUNTIME -i oraclelinux bash -c 'for NUM in `seq 1 1 10`; do touch $NUM-file.txt && ls -l /$NUM-file.txt; done'
}

@test "[java application] run hello world in java in an openjdk container" {
	docker run --rm --runtime=$RUNTIME -i openjdk bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java && javac HW.java && java HW"
}

@test "[display text] run an opensuse container" {
	docker run --rm --runtime=$RUNTIME -i opensuse sh -c "echo "testing" > test.txt | cat /test.txt | grep testing"
}

@test "[start application] start orientdb server" {
	docker run --runtime=$RUNTIME -e ORIENTDB_ROOT_PASSWORD=mysecretword -d orientdb timeout -t 10 /orientdb/bin/server.sh
}

@test "[perl application] start instance in percona container" {
	if docker run --rm --runtime=$RUNTIME -i percona perl -e 'print "Kata Containers\n"' | grep LANG; then false; else true; fi
}

@test "[php application] run php container" {
	docker run --rm --runtime=$RUNTIME -i php sh -c "php -r 'print(\"kata-runtime\");'"
}

@test "[display configuration] check build number of a photon container" {
	docker run --rm --runtime=$RUNTIME -i photon cat /etc/photon-release
}

@test "[display text] print a piwik container" {
	docker run --rm --runtime=$RUNTIME -i piwik bash -c "php -r 'print(\"Kata Containers\");'"
}

@test "[python application] execute a pypy container" {
	docker run --rm --runtime=$RUNTIME -i pypy bash -c "python -m timeit -s \"M=range(1000);f=lambda x:x*2\" \"L=[f(m) for m in M]"\"
}

@test "[python application] run python container" {
	docker run --rm --runtime=$RUNTIME -i python sh -c "python -m timeit -s \"M=range(1000);f=lambda x: x*2\" \"L=map(f,M)\""
}

@test "[display configuration] start a rabbitmq container" {
	docker run --runtime=$RUNTIME --hostname my-rabbit-container -e RABBITMQ_DEFAULT_USER=testuser -e RABBITMQ_DEFAULT_PASS=mysecretword -d rabbitmq rabbitmqctl reset
}

@test "[display text] print message in a r-base container" {
	docker run --rm --runtime=$RUNTIME r-base r -e 'print ( "Hello World!")'
}

@test "[start application] create rails application" {
	docker run --rm --runtime=$RUNTIME -i rails timeout 10 rails new commandsapp | grep create
}

@test "[perl application] run rakudo star container" {
	if docker run --rm --runtime=$RUNTIME -i rakudo-star perl -e 'print "Hello\n"' | grep "LANG"; then false; else true; fi
}

@test "[display configuration] start redis server with a certain port" {
	docker run --rm --runtime=$RUNTIME -i redis sh -c "timeout 10 redis-server --port 7777 | grep 7777"
}

@test "[display configuration] start redis server" {
	docker run --rm --runtime=$RUNTIME -i redis sh -c "timeout 5 redis-server --appendonly yes | grep starting"
}

@test "[run application] search gcc in a ros container" {
	docker run --rm --runtime=$RUNTIME -i ros apt-cache search gcc
}

@test "[ruby application] print message in a ruby container" {
	docker run --rm --runtime=$RUNTIME -i ruby sh -c "ruby -e \"puts 'Kata Containers'\""
}

@test "[display configuration] generate key in a sentry container" {
	docker run --rm --runtime=$RUNTIME -i sentry config generate-secret-key
}

@test "[display configuration] start solr server" {
	docker run --rm --runtime=$RUNTIME -i solr timeout 10 solr -h
}

@test "[swarm create] start swarm container" {
	if docker run --runtime=$RUNTIME -i swarm create | grep "EXEC spawning"; then false; else true; fi
}

@test "[run application] generate a telegraf conf file" {
	docker run --rm --runtime=$RUNTIME -i telegraf sh -c "telegraf config > telegraf.conf; ls telegraf.conf"
}

@test "[java application] run a tomcat container" {
	docker run --rm --runtime=$RUNTIME -i tomcat sh -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java | ls -l /usr/local/tomcat/HW.java"
}

@test "[java application] run tomee container" {
	docker run --rm --runtime=$RUNTIME -i tomee bash -c "echo -e 'public class CL{public static void main(String[]a){System.out.println(\"KataContainers\");}}' > CL.java"
}

@test "[display version] run an instance in a traefik container" {
	if docker run --rm --runtime=$RUNTIME -i traefik traefik --version | grep "EXEC spawning"; then false; else true; fi
}

@test "[run application] run an instance in an ubuntu debootstrap container" {
	docker run --rm --runtime=$RUNTIME -i ubuntu-debootstrap sh -c 'if [ -f /etc/bash.bashrc ]; then echo "/etc/bash.bashrc exists"; fi'
}

@test "[run application] search nano in an ubuntu upstart container" {
	docker run --rm --runtime=$RUNTIME -i ubuntu-upstart bash -c "apt-cache search nano"
}

@test "[run application] start server in a vault container" {
	docker run --rm --runtime=$RUNTIME -i -e 'VAULT_DEV_ROOT_TOKEN_ID=mytest' vault timeout -t 10 vault server -dev
}

@test "[perl application] start wordpress container" {
	if docker run --rm --runtime=$RUNTIME -i wordpress perl -e 'print "test\n"' | grep LANG; then false; else true; fi
}

@test "[run application] start zookeeper container" {
	docker run --rm --runtime=$RUNTIME -i zookeeper zkServer.sh start
}

teardown() {
	clean_env
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
