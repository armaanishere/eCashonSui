CREATE TABLE EVENTS (
    id BIGSERIAL PRIMARY KEY,
    transaction_digest VARCHAR(255) NOT NULL,
    event_sequence BIGINT NOT NULL,
    event_time TIMESTAMP,
    event_type VARCHAR NOT NULL,
    event_content VARCHAR NOT NULL,
    next_cursor_transaction_digest VARCHAR(255)
);
