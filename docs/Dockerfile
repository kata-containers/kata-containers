FROM python:3.12-slim

COPY ./requirements.txt requirements.txt
RUN pip install -r requirements.txt

ENTRYPOINT ["python3", "-m", "mkdocs"]
