//! `DbmsAdapter` implementations. All SQL string literals live under here
//! (the `check-no-sql-outside-adapters.sh` grep gate already permits
//! `core/src/dbms/adapters/`, unchanged since U0).

pub mod postgresql;
