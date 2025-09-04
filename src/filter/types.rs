use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    #[serde(rename = "$eq")] Eq,
    #[serde(rename = "$ne")] Ne,
    #[serde(rename = "$neq")] Neq,
    #[serde(rename = "$gt")] Gt,
    #[serde(rename = "$gte")] Gte,
    #[serde(rename = "$lt")] Lt,
    #[serde(rename = "$lte")] Lte,

    #[serde(rename = "$like")] Like,
    #[serde(rename = "$nlike")] NLike,
    #[serde(rename = "$ilike")] ILike,
    #[serde(rename = "$nilike")] NILike,
    #[serde(rename = "$regex")] Regex,
    #[serde(rename = "$nregex")] NRegex,

    #[serde(rename = "$in")] In,
    #[serde(rename = "$nin")] NIn,

    #[serde(rename = "$any")] Any,
    #[serde(rename = "$all")] All,
    #[serde(rename = "$nany")] NAny,
    #[serde(rename = "$nall")] NAll,
    #[serde(rename = "$size")] Size,

    #[serde(rename = "$and")] And,
    #[serde(rename = "$or")] Or,
    #[serde(rename = "$not")] Not,
    #[serde(rename = "$nand")] NAnd,
    #[serde(rename = "$nor")] NOr,

    #[serde(rename = "$between")] Between,

    #[serde(rename = "$find")] Find,
    #[serde(rename = "$text")] Text,

    #[serde(rename = "$exists")] Exists,
    #[serde(rename = "$null")] Null,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterData {
    pub select: Option<Vec<String>>,
    pub where_clause: Option<serde_json::Value>,
    pub order: Option<serde_json::Value>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FilterWhereInfo {
    pub column: String,
    pub operator: FilterOp,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct FilterWhereOptions {
    pub include_trashed: bool,
    pub include_deleted: bool,
}

impl Default for FilterWhereOptions {
    fn default() -> Self {
        Self {
            include_trashed: false,
            include_deleted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FilterOrderInfo {
    pub column: String,
    pub sort: SortDirection,
}

#[derive(Debug, Clone)]
pub struct SqlResult {
    pub query: String,
    pub params: Vec<serde_json::Value>,
}
