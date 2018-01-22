FROM euleros:2.2

ADD https://storage.googleapis.com/golang/go1.9.2.linux-amd64.tar.gz /tmp
RUN tar -C /usr/ -xzf /tmp/go1.9.2.linux-amd64.tar.gz
RUN yum install -y make
ENV GOROOT=/usr/go
ENV PATH=$PATH:$GOROOT/bin:$GOPATH/bin
