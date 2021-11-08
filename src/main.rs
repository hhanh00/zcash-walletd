#[macro_use] extern crate rocket;

mod rpc;
mod db;

pub use crate::rpc::*;

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![
        create_account, create_address, get_account,
        get_transaction, make_payment,
    ])
}
