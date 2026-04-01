fn main() {
    // CXX-Qt build script
    cxx_qt_build::CxxQtBuild::new()
        .with_moc("cpp/app.h")
        .build();
}
