#[macro_export]
macro_rules! ReqRes {
    {
        $(
        $(#[$attr:meta])*
        $name:ident {
            $(
            $(#[$req_field_attr:meta])*
            $req_field_name:ident: $req_field_ty:ty
            ),*
            $(,)?
            =>
            $(
            $(#[$res_field_attr:meta])*
            $res_field_name:ident: $res_field_ty:ty
            ),*
            $(,)?
        }
        ),*
        $(,)?
    } => (
    #[derive(Clone, Copy, Debug, IsVariant, PartialEq, Eq)]
    #[repr(C)]
    #[non_exhaustive]
    pub enum Request {
    $(
    $(#[$attr])*
    $name {
        $(
        $(#[$req_field_attr])*
        $req_field_name: $req_field_ty
        ),*
    }),*
    }

    #[derive(Clone, Copy, Debug, IsVariant, PartialEq, Eq)]
    #[repr(C)]
    #[non_exhaustive]
    pub enum Response {
    $(
    $name {
        $(
        $(#[$res_field_attr])*
        $res_field_name: $res_field_ty
        ),*

    }),*
    }
    )
}
