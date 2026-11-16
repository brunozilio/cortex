create table preferences (
    key text primary key,
    value jsonb not null,
    updated_at timestamptz not null default now()
);
