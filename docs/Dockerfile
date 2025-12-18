FROM python:3

RUN apt-get update
RUN apt-get install -y libcairo2-dev libfreetype6-dev libffi-dev libjpeg-dev libpng-dev libz-dev pngquant 
RUN apt-get install -y python3-pip python3-venv
WORKDIR /build
COPY requirements.txt .
RUN pip install -r ./requirements.txt

ENTRYPOINT ["mkdocs"]
