create table instance (
    id smallint primary key default 1 check (id = 1),
    created_at timestamptz not null default now()
);

insert into instance (id) values (1);
