use serde::Serialize;

#[derive(Serialize)]
pub struct EIDEConfigContext<'a> {
    pub project_name: &'a String,
    pub ld_file_path: &'a String,
    pub src_dirs: &'a String,
    pub include_list: &'a String,
    pub define_list: &'a String,
    pub src_files: &'a String,
}

#[derive(Serialize)]
pub struct CreateContext<'a> {
    pub project_name: &'a String,
    pub project_dir: &'a String,
    pub ioc_file_path: &'a String,
    pub toolchain: &'a str,
    pub generate_under_root: bool,
}
