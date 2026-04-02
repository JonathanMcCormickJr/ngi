#![allow(clippy::all, clippy::pedantic)]

pub mod admin {
    tonic::include_proto!("admin");
}

pub mod auth {
    tonic::include_proto!("auth");
}

pub mod chaos {
    tonic::include_proto!("chaos");
}

pub mod custodian {
    tonic::include_proto!("custodian");
}

pub mod db {
    tonic::include_proto!("db");
}
