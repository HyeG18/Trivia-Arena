fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Le decimos a tonic que compile el archivo user.proto
    tonic_build::compile_protos("../../proto/user.proto")?;
    Ok(())
}