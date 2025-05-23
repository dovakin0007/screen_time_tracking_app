extern crate diesel;
extern crate diesel_migrations;
use build_print::println;
use diesel::{sqlite::SqliteConnection, Connection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use std::env;
use std::fs;
use std::path::PathBuf;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db_path = if db_url.contains("%AppData%") {
        let app_data_path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        db_url.replace("%AppData%", app_data_path.to_str().unwrap())
    } else {
        db_url
    };

    let db_path = PathBuf::from(db_path);

    if let Some(parent_dir) = db_path.parent() {
        fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    }

    println!("{:?}", db_path);
    SqliteConnection::establish(db_path.to_str().unwrap())
        .unwrap_or_else(|_| panic!("Error connecting to {}", db_path.display()))
}

fn run_migrations(connection: &mut SqliteConnection) {
    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}

fn main() {
    let mut connection = establish_connection();
    println!("Database connection established successfully!");
    run_migrations(&mut connection);
    println!("cargo:rerun-if-changed=dist");

    // Run the Tauri build-time helpers
    tauri_build::build()
    //     let windows = tauri_build::WindowsAttributes::new().app_manifest(
    //         r#"
    //     <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
    //   <dependency>
    //     <dependentAssembly>
    //       <assemblyIdentity
    //         type="win32"
    //         name="Microsoft.Windows.Common-Controls"
    //         version="6.0.0.0"
    //         processorArchitecture="*"
    //         publicKeyToken="6595b64144ccf1df"
    //         language="*"
    //       />
    //     </dependentAssembly>
    //   </dependency>
    //   <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    //     <security>
    //         <requestedPrivileges>
    //             <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
    //         </requestedPrivileges>
    //     </security>
    //   </trustInfo>
    // </assembly>
    //     "#,
    //     );
    //     let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    //     tauri_build::try_build(attrs).expect("build failed");
}
