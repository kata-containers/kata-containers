#!/usr/bin/env bats
# *-*- Mode: sh; sh-basic-offset: 8; indent-tabs-mode: nil -*-*
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for the most popular images from docker hub.

source ${BATS_TEST_DIRNAME}/../../lib/common.bash

versions_file="${BATS_TEST_DIRNAME}/../../versions.yaml"
kibana_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.kibana.version")
kibana_image="kibana:$kibana_version"

setup() {
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
	clean_env
}

@test "[insert data] insert data in an aerospike container" {
	image="aerospike/aerospike-server"
	docker run -m 6G --runtime=$RUNTIME -d --name aerospike $image
	status=1
	set +e
	for i in $(seq 1 5); do
		image2="aerospike/aerospike-tools"
		docker run --rm --runtime=$RUNTIME -i $image2 aql -h $(docker inspect -f '{{.NetworkSettings.IPAddress}}' aerospike) -c "insert into test.foo (PK, foo) values ('123','any'); select * from test.foo"
		if [ $? == 0 ]; then
			status=0
			break
		fi
		sleep 1
	done
	set -e
	docker rm -f aerospike
	docker rmi $image2
	return $status
}

@test "[display text] hello world in an alpine container" {
	image="alpine"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "echo 'Hello, World'"
}

@test "[display release] check os version in an alpine container" {
	image="alpine"
	docker run --rm --runtime=$RUNTIME $image cat /etc/alpine-release
}

@test "[run application] run an amazonlinux container" {
	image="amazonlinux"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "amazon-linux-extras install -y firecracker; firecracker --help"
}

@test "[display version] run an arangodb container" {
	image="arangodb/arangodb"
	docker run --rm --runtime=$RUNTIME -e ARANGO_ROOT_PASSWORD=secretword -e ARANGO_NO_AUTH=1 -p 8529:8529 $image foxx-manager --version
}

@test "[display help] search job_spec in the  help option of a bonita container" {
	image="bonita"
	docker run --rm --runtime=$RUNTIME -i -e "TENANT_LOGIN=testuser" -e "TENANT_PASSWORD=mysecretword" $image bash -c "help | grep job"
}

@test "[display version] show the os version inside a buildpack deps container" {
	image="buildpack-deps"
	docker run --rm --runtime=$RUNTIME -i $image cat /etc/os-release
}

@test "[display version] display the cqlsh version in a cassandra container" {
	image="cassandra"
	docker run --rm --runtime=$RUNTIME -i $image cqlsh --version
}

@test "[python operation] run a celery container" {
	image="celery"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "python -m timeit \"[i for i in range(1000)]"\"
}

@test "[create file] cat a file in a centos container" {
	image="centos"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo "Test" > testfile.txt | cat testfile.txt | grep Test"
}

@test "[display version] display options in a chronograf container" {
	image="chronograf"
	docker run --rm --runtime=$RUNTIME -i $image --version
}

@test "[find bundle] check os-core-update bundle in a clearlinux container" {
	image="clearlinux"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "ls /usr/share/clear/bundles | grep os-core-update"
}

@test "[display hostname] check hostname in a clearlinux container" {
	image="clearlinux"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "hostname"
}

@test "[run java] execute a message in a clojure container" {
	image="clojure"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo -e 'public class CC{public static void main(String[]a){System.out.println(\"KataContainers\");}}' > CC.java && javac CC.java && java CC"
}

@test "[create file] run a couchbase container" {
	image="couchbase"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "mkdir /home/test; ls /home | grep test"
}

@test "[run agent] run a consul container" {
	image="consul"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "timeout -t 10 consul agent -dev -client 0.0.0.0 | grep 0.0.0.0"
}

@test "[display version] run a crate container" {
	image="crate"
	docker run --rm --runtime=$RUNTIME -i -e CRATE_HEAP_SIZE=1g $image timeout 10 crate -v
}

@test "[display nameserver] check the resolv.conf in a crux container" {
	image="crux"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "cat /etc/resolv.conf | grep nameserver"
}

@test "[display credentials] instance in a django container" {
	image="django"
	docker run --rm --runtime=$RUNTIME -i --user "$(id -u):$(id -g)" $image sh -c "django-admin | grep sqlflush"
}

@test "[command options] run an instance in a docker container" {
	image="docker"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "docker inspect --help"
}

@test "[run application] run an instance in an eclipse-mosquitto container" {
	image="eclipse-mosquitto"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "mosquitto -d"
}

@test "[display directory] run an instance in an elixir container" {
	image="elixir"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "pwd"
}

@test "[start configuration] run an erlang container" {
	image="erlang"
	docker run --runtime=$RUNTIME -d $image erl -name test@erlang.local
}

@test "[display time] date in a fedora container" {
	image="fedora"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "date"
}

@test "[display version] search python version in a fedora/tools container" {
	image="fedora/tools"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "python --version"
}

@test "[command options] find the timestamp help in a gazebo container" {
	image="gazebo"
	docker run --rm --runtime=$RUNTIME -i $image gz log --help
}

@test "[gcc file] run a gcc container" {
	image="gcc"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo -e '#include<stdio.h>\nint main (void)\n{printf(\"Hello\");return 0;}' > demo.c && gcc demo.c -o demo && ./demo"
}


@test "[gradle] run a gradle container" {
	image="gradle"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo -e 'public class CL{public static void main(String[]a){System.out.println(\"KataContainers\");}}' > CL.java && javac CL.java && java CL"
}

@test "[groovy] run a groovy container" {
	image="groovy"
	docker run --runtime=$RUNTIME --rm -i -e hola=caracol $image bash -c "groovy -e \"println System.getenv().each{println it}\" | grep 'hola=caracol'"
}

@test "[java file] run an instance in a glassfish container" {
	image="glassfish"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo 'public class T{public static void main(String[]a){System.out.println(\"Test\");}}' > T.java && javac T.java && java T"
}

@test "[golang file] run golang container" {
	image="golang"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo -e 'package main\nimport \"fmt\"\nfunc main() { fmt.Println(\"hello world\")}' > hello-world.go && go run hello-world.go && go build hello-world.go"
}

@test "[golang settings] go env in a golang container" {
	image="golang"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "go env | grep GOARCH"
}

@test "[memory settings] set memory size in haproxy container" {
	image="haproxy"
	docker run --rm --runtime=$RUNTIME -i $image haproxy -m 2 -v
}

@test "[display version] run haskell container" {
	image="haskell"
	docker run --rm --runtime=$RUNTIME -i $image cabal --numeric-version
}

@test "[display text] run hello world container" {
	image="hello-world"
	docker run --rm --runtime=$RUNTIME -i $image | grep "Hello from Docker"
}

@test "[display text] run hello seattle container" {
	image="hello-seattle"
	docker run --rm --runtime=$RUNTIME -i $image | grep "Hello from DockerCon"
}

@test "[run application] start apachectl in a httpd container" {
	image="httpd"
	if docker run --rm --runtime=$RUNTIME -i $image apachectl -k start | grep "Unable to open logs"; then false; else true; fi
}

@test "[python application] run python command in a hylang container" {
	image="hylang"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "python -m timeit -s \"L=[]; M=range(1000)\" \"for m in M: L.append(m*2)\""
}

@test "[configuration settings] display config information of a influxdb container" {
	image="influxdb"
	docker run --rm --runtime=$RUNTIME -i $image influxd config
}

@test "[create directory] start an instance in iojs container" {
	image="iojs"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "mkdir /root/test; ls /root | grep test"
}

@test "[configuration settings] set nick in an irssi container" {
	image="irssi"
	docker run -d --runtime=$RUNTIME $image irssi -n test
}

@test "[java application] display configuration parameters in a jetty container" {
	image="jetty"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java; ls -l ./HW.java"
}

@test "[display version] run jetty container" {
	image="jetty"
	docker run --rm --runtime=$RUNTIME -i $image --version
}

@test "[ruby application] start jruby container" {
	image="jruby"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "jruby -e \"puts 'Containers'\""
}

@test "[julia application] display message in a julia container" {
	image="julia"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "julia -e 'println(\"this is a test\")'"
}

@test "[display configuration] run kapacitor container" {
	image="kapacitor"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "kapacitord config > kapacitor.conf | ls -l kapacitor.conf"
}

@test "[display version] display information kibana container" {
	image=$kibana_image
	docker run --rm --runtime=$RUNTIME -i $kibana_image kibana --version
}

@test "[display configuration] check kong configuration file is valid" {
	image="kong"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "kong check /etc/kong/kong.conf.default | grep valid"
}

@test "[display kernel] check kernel version in a mageia container" {
	image="mageia"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "uname -a | grep Linux"
}

@test "[display configuration] start an instance of a mariadb container" {
	image="mariadb"
	docker run --rm --runtime=$RUNTIME -i -e MYSQL_ROOT_PASSWORD=secretword  $image bash -c "cat /etc/mysql/mariadb.cnf | grep character"
}

@test "[matomo] run a matomo container" {
	image="matomo"
	docker run --runtime=$RUNTIME --rm -i $image bash -c "php -r 'print(\"Kata Containers\");'"
}

@test "[java application] check memory maven container" {
	image="maven"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java"
}

@test "[perl application] run memcached container" {
	image="memcached"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "perl -e 'print \"memcachedtest\n\"'"
}

@test "[display version] start mongo container" {
	image="mongo"
	docker run --rm --runtime=$RUNTIME -i $image --version
}

@test "[display version] start nats server" {
	image="nats"
	docker run --rm --runtime=$RUNTIME -i $image --version
}

@test "[display text] create a file in a neo4j container" {
	image="neo4j"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "echo "running" > test; cat /var/lib/neo4j/test"
}

@test "[display configuration] configuration file neurodebian" {
	image="neurodebian"
	docker run --rm --runtime=$RUNTIME $image cat /etc/apt/sources.list.d/neurodebian.sources.list
}

@test "[nextcloud] run nextcloud container" {
	image="nextcloud"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "php -r 'print(\"kata-runtime\");'"
}

@test "[display configuration] run nginx container" {
	image="nginx"
	docker run --rm --runtime=$RUNTIME $image dpkg-query -l | grep --color=no "libc"
}

@test "[display configuration] search in a node container" {
	image="node"
	docker run --rm --runtime=$RUNTIME -i $image node --v8-options
}

@test "[display search] search in a nuxeo container" {
	image="nuxeo"
	docker run --rm --runtime=$RUNTIME -i $image apt-cache search python
}

@test "[perl application] run an odoo container" {
	image="odoo"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "perl -e 'print "Hello\n"'"
}

@test "[create files] create files in an oraclelinux container" {
	image="oraclelinux"
	docker run --rm --runtime=$RUNTIME -i $image bash -c 'for NUM in `seq 1 1 10`; do touch $NUM-file.txt && ls -l /$NUM-file.txt; done'
}

@test "[java application] run hello world in java in an openjdk container" {
	image="openjdk"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java && javac HW.java && java HW"
}

@test "[display text] run an opensuse leap container" {
	image="opensuse/leap"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "echo "testing" > test.txt | cat /test.txt | grep testing"
}

@test "[start application] start orientdb server" {
	image="orientdb"
	docker run --runtime=$RUNTIME -e ORIENTDB_ROOT_PASSWORD=mysecretword -d $image timeout -t 10 /orientdb/bin/server.sh
}

@test "[perl application] start instance in percona container" {
	image="percona"
	if docker run --rm --runtime=$RUNTIME -i $image perl -e 'print "Kata Containers\n"' | grep LANG; then false; else true; fi
}

@test "[php application] run php container" {
	image="php"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "php -r 'print(\"kata-runtime\");'"
}

@test "[display configuration] check build number of a photon container" {
	image="photon"
	docker run --rm --runtime=$RUNTIME -i $image cat /etc/photon-release
}

@test "[display text] print a piwik container" {
	image="piwik"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "php -r 'print(\"Kata Containers\");'"
}

@test "[python application] execute a pypy container" {
	image="pypy"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "python -m timeit -s \"M=range(1000);f=lambda x:x*2\" \"L=[f(m) for m in M]"\"
}

@test "[python application] run python container" {
	image="python"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "python -m timeit -s \"M=range(1000);f=lambda x: x*2\" \"L=map(f,M)\""
}

@test "[display configuration] start a rabbitmq container" {
	image="rabbitmq"
	docker run --runtime=$RUNTIME --hostname my-rabbit-container -e RABBITMQ_DEFAULT_USER=testuser -e RABBITMQ_DEFAULT_PASS=mysecretword -d $image rabbitmqctl reset
}

@test "[display text] print message in a r-base container" {
	image="r-base"
	docker run --rm --runtime=$RUNTIME $image r -e 'print ( "Hello World!")'
}

@test "[perl application] run rakudo star container" {
	image="rakudo-star"
	if docker run --rm --runtime=$RUNTIME -i $image perl -e 'print "Hello\n"' | grep "LANG"; then false; else true; fi
}

@test "[display configuration] start redis server with a certain port" {
	image="redis"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "timeout 10 redis-server --port 7777 | grep 7777"
}

@test "[display configuration] start redis server" {
	image="redis"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "timeout 5 redis-server --appendonly yes | grep starting"
}

@test "[run application] search gcc in a ros container" {
	image="ros"
	docker run --rm --runtime=$RUNTIME -i $image apt-cache search gcc
}

@test "[ruby application] print message in a ruby container" {
	image="ruby"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "ruby -e \"puts 'Kata Containers'\""
}

@test "[display configuration] generate key in a sentry container" {
	image="sentry"
	docker run --rm --runtime=$RUNTIME -i $image config generate-secret-key
}

@test "[display configuration] start solr server" {
	image="solr"
	docker run --rm --runtime=$RUNTIME -i $image timeout 10 solr -h
}

@test "[swarm create] start swarm container" {
	image="swarm"
	if docker run --runtime=$RUNTIME -i $image create | grep "EXEC spawning"; then false; else true; fi
}

@test "[run application] generate a telegraf conf file" {
	image="telegraf"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "telegraf config > telegraf.conf; ls telegraf.conf"
}

@test "[java application] run a tomcat container" {
	image="tomcat"
	docker run --rm --runtime=$RUNTIME -i $image sh -c "echo 'public class HW{public static void main(String[]a){System.out.println(\"HelloWorld\");}}' > HW.java | ls -l /usr/local/tomcat/HW.java"
}

@test "[java application] run tomee container" {
	image="tomee"
	docker run --rm --runtime=$RUNTIME -i $image bash -c "echo -e 'public class CL{public static void main(String[]a){System.out.println(\"KataContainers\");}}' > CL.java"
}

@test "[display version] run an instance in a traefik container" {
	image="traefik"
	if docker run --rm --runtime=$RUNTIME -i $image traefik --version | grep "EXEC spawning"; then false; else true; fi
}

@test "[teamspeak] run a teamspeak container" {
	image="teamspeak"
	docker run --rm --runtime=$RUNTIME -i -p 9987:9987/udp -p 10011:10011 -p 30033:30033 -e TS3SERVER_LICENSE=accept $image sh -c "printf 'Kata Containers'"
}

@test "[run application] run an instance in an ubuntu debootstrap container" {
	image="ubuntu-debootstrap"
	docker run --rm --runtime=$RUNTIME -i $image sh -c 'if [ -f /etc/bash.bashrc ]; then echo "/etc/bash.bashrc exists"; fi'
}

@test "[run application] start server in a vault container" {
	image="vault"
	docker run --rm --runtime=$RUNTIME -i -e 'VAULT_DEV_ROOT_TOKEN_ID=mytest' $image timeout 10 vault server -dev
}

@test "[perl application] start wordpress container" {
	image="wordpress"
	if docker run --rm --runtime=$RUNTIME -i $image perl -e 'print "test\n"' | grep LANG; then false; else true; fi
}

@test "[run application] start zookeeper container" {
	image="zookeeper"
	docker run --rm --runtime=$RUNTIME -i $image zkServer.sh
}

teardown() {
	clean_env
	docker rmi $image
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
