CREATE TABLE processes (
    id SERIAL PRIMARY KEY,
    executable VARCHAR NOT NULL,
    name VARCHAR NULL,
    export BOOLEAN DEFAULT true NOT NULL
);

CREATE UNIQUE INDEX unique_process ON processes (executable, name);

CREATE TABLE events (
    id SERIAL PRIMARY KEY,
    time TIMESTAMPTZ NOT NULL,
    process INTEGER REFERENCES processes(id) NOT NULL,
    duration INTERVAL NOT NULL
);
