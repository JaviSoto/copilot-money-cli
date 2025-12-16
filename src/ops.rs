pub const USER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/graphql/User.graphql"));
pub const TRANSACTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/Transactions.graphql"
));
pub const CATEGORIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/Categories.graphql"
));
pub const RECURRINGS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/Recurrings.graphql"
));
pub const BUDGETS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/Budgets.graphql"
));
