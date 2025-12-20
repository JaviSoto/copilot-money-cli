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
pub const TAGS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/graphql/Tags.graphql"));
pub const BUDGETS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/Budgets.graphql"
));

pub const BULK_EDIT_TRANSACTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/BulkEditTransactions.graphql"
));
pub const EDIT_TRANSACTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/EditTransaction.graphql"
));
pub const ADD_TRANSACTION_TO_RECURRING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/AddTransactionToRecurring.graphql"
));
pub const CREATE_TAG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/CreateTag.graphql"
));
pub const CREATE_CATEGORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/CreateCategory.graphql"
));
pub const CREATE_RECURRING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/CreateRecurring.graphql"
));
pub const EDIT_RECURRING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/EditRecurring.graphql"
));
pub const DELETE_TAG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/graphql/DeleteTag.graphql"
));
