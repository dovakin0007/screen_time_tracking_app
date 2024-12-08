#[cfg(target_os = "windows")]
extern crate winres;

fn main() {
    tonic_build::compile_protos("proto/send_user_data.proto")
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));

    if cfg!(target_os = "windows") {
        use std::io::Write;
        if std::env::var("PROFILE").unwrap() == "release" {
            let mut res = winres::WindowsResource::new();
            res.set_manifest(
                r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
        <requestedPrivileges>
            <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
        </requestedPrivileges>
    </security>
</trustInfo>
</assembly>
"#,
            );
            match res.compile() {
                Err(error) => {
                    write!(std::io::stderr(), "{}", error).unwrap();
                    std::process::exit(1);
                }
                Ok(_) => {}
            }
        }
    }
}
