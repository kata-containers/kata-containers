FROM python:3.12-slim

WORKDIR /
COPY ./requirements.txt requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

ENTRYPOINT ["python3", "-m", "mkdocs"]
