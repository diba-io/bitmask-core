mod rgb {

    mod unit {
        mod invoice;
        mod issue;
        mod psbt;
        mod stock;
        pub mod utils;
    }

    mod integration {
        mod import;
        mod issue;
        mod transfer;
        pub mod utils;
    }
}
