use postgres::Client;

pub const MIGRATIONS: &[(&str, &str)] = &[("0001_instance", include_str!("../migrations/0001_instance.sql"))];

pub fn apply(client: &mut Client) -> Result<(), postgres::Error> {
    client.batch_execute(
        "create table if not exists store_migrations (name text primary key, applied_at timestamptz not null default now())",
    )?;
    for (name, sql) in MIGRATIONS {
        let mut transaction = client.transaction()?;
        transaction.execute("lock table store_migrations in exclusive mode", &[])?;
        let applied = transaction.query_opt("select 1 from store_migrations where name = $1", &[name])?.is_some();
        if !applied {
            transaction.batch_execute(sql)?;
            transaction.execute("insert into store_migrations (name) values ($1)", &[name])?;
        }
        transaction.commit()?;
    }
    Ok(())
}
