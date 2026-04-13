use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
#[archive(check_bytes)]
pub struct ApiSpec {
    pub operations: Vec<ApiOperation>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
#[archive(check_bytes)]
pub struct ApiOperation {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub tag: Option<String>,
    pub parameters: Vec<ApiParam>,
    pub has_request_body: bool,
    /// Resolved JSON schema for the request body (pretty-printed), if any.
    pub request_body_schema: Option<String>,
    /// Body schema has a top-level `page` property — synthesize `--page` flag.
    pub body_has_page: bool,
    /// Body schema has a top-level `pageSize` property — synthesize `--page-size` flag.
    pub body_has_page_size: bool,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
#[archive(check_bytes)]
pub struct ApiParam {
    /// camelCase name from spec
    pub name: String,
    pub location: ParamLocation,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, PartialEq)]
#[archive(check_bytes)]
pub enum ParamLocation {
    Path,
    Query,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
#[archive(check_bytes)]
pub struct CachedSite {
    pub id: String,
    pub name: String,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug)]
#[archive(check_bytes)]
pub struct SiteList {
    pub sites: Vec<CachedSite>,
}
