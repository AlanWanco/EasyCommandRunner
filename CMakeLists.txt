cmake_minimum_required(VERSION 3.24)

project(EasyCommandRunner VERSION 0.9.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Enable Qt6 auto-moc
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# Find Qt6
find_package(Qt6 COMPONENTS
    Core
    Gui
    Widgets
    Network
    Sql
    Concurrent
    Svg
    REQUIRED
)

# For now, we'll build just the C++ frontend without CXX-Qt integration
# The Rust backend will be integrated via FFI in a later phase
# To build Rust: cargo build [--release]

# Determine the Rust target library name and path
if(APPLE)
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/libeasy_command_runner.a")
elseif(WIN32)
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/easy_command_runner.lib")
else()
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/libeasy_command_runner.a")
endif()

# Make the Rust archive an explicit build output.  This is required by Ninja
# on a clean checkout; an order-only dependency on a custom target is not
# enough when the archive is also a link input.
file(GLOB_RECURSE RUST_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs"
)
add_custom_command(
    OUTPUT ${RUST_LIB_PATH}
    COMMAND cargo build
            --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
            $<IF:$<CONFIG:Release>,--release,>
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    DEPENDS
        ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
        ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock
        ${RUST_SOURCES}
    COMMENT "Building Rust backend"
    VERBATIM
)
add_custom_target(rust_build ALL DEPENDS ${RUST_LIB_PATH})

# Source files
set(SOURCES
    cpp/app.cpp
    cpp/log_panel.cpp
    cpp/parameter_drag_handle.cpp
)

set(HEADERS
    cpp/app.h
    cpp/log_panel.h
    cpp/parameter_drag_handle.h
)

# Resources
set(RESOURCES
    resources/resources.qrc
)
if(WIN32)
    # Qt's setWindowIcon() only affects the running window.  Embed an icon
    # resource as well so Explorer and shortcuts use the same application icon.
    set(PLATFORM_RESOURCES
        resources/app_icon.rc
    )
elseif(APPLE)
    # Put the native icon into the .app bundle; the PNG in the Qt resource
    # file is only used at runtime and does not set the bundle icon.
    set(PLATFORM_RESOURCES
        resources/app_icon.icns
    )
    set_source_files_properties(${PLATFORM_RESOURCES} PROPERTIES
        MACOSX_PACKAGE_LOCATION Resources
    )
endif()

# Create executable
add_executable(${PROJECT_NAME}
    ${SOURCES}
    ${HEADERS}
    ${RESOURCES}
    ${PLATFORM_RESOURCES}
)

# Link libraries
target_link_libraries(${PROJECT_NAME}
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Network
    Qt6::Sql
    Qt6::Concurrent
    Qt6::Svg
    ${RUST_LIB_PATH}
)
target_compile_definitions(${PROJECT_NAME}
    PRIVATE
    "ECR_APPLICATION_VERSION=\"${PROJECT_VERSION}\""
)

# Rust's Windows process implementation imports NtCreateNamedPipeFile
# from ntdll; static libraries do not propagate this system dependency.
if(WIN32)
    target_link_libraries(${PROJECT_NAME} ntdll)
endif()

# Add dependency on Rust build
add_dependencies(${PROJECT_NAME} rust_build)

# Include directories
target_include_directories(${PROJECT_NAME}
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/cpp
    ${CMAKE_CURRENT_BINARY_DIR}
)

option(ECR_BUILD_TESTS "Build Qt UI and process integration tests" OFF)
if(ECR_BUILD_TESTS)
    enable_testing()
    find_package(Qt6 COMPONENTS Test REQUIRED)
    add_executable(ecr_ui_tests tests/ui_test.cpp ${SOURCES} ${HEADERS} ${RESOURCES})
    target_compile_definitions(ecr_ui_tests PRIVATE ECR_NO_MAIN)
    target_include_directories(ecr_ui_tests PRIVATE cpp)
    target_link_libraries(ecr_ui_tests PRIVATE Qt6::Test Qt6::Widgets Qt6::Svg ${RUST_LIB_PATH})
    if(WIN32)
        target_link_libraries(ecr_ui_tests PRIVATE ntdll)
    endif()
    add_dependencies(ecr_ui_tests rust_build)
    add_test(NAME ecr_ui_tests COMMAND ecr_ui_tests)
    if(WIN32)
        # Qt's Windows offscreen backend crashes before QTest starts on the
        # MSVC build; the hosted runner provides a usable Windows desktop.
        set(ECR_TEST_QPA_PLATFORM windows)
    else()
        set(ECR_TEST_QPA_PLATFORM offscreen)
    endif()
    set_tests_properties(ecr_ui_tests PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=${ECR_TEST_QPA_PLATFORM};QT_SCALE_FACTOR=1"
        TIMEOUT 180)
endif()

# Platform-specific settings
if(WIN32)
    set_target_properties(${PROJECT_NAME} PROPERTIES WIN32_EXECUTABLE ON)
    if(MSVC)
        # Visual Studio settings
        set_target_properties(${PROJECT_NAME} PROPERTIES MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
    endif()
elseif(APPLE)
    set_target_properties(${PROJECT_NAME} PROPERTIES
        MACOSX_BUNDLE ON
        MACOSX_BUNDLE_BUNDLE_NAME "EasyCommandRunner"
        MACOSX_BUNDLE_BUNDLE_VERSION "${PROJECT_VERSION}"
        MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}"
        MACOSX_BUNDLE_ICON_FILE "app_icon.icns"
    )
endif()

# Install targets
install(TARGETS ${PROJECT_NAME}
    BUNDLE DESTINATION . COMPONENT Runtime
    RUNTIME DESTINATION bin COMPONENT Runtime
)

# CPack configuration
set(CPACK_PACKAGE_NAME "EasyCommandRunner")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_VENDOR "SleepyKanata")
set(CPACK_PACKAGE_DESCRIPTION "A modern cross-platform command runner with GUI")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "EasyCommandRunner")

if(WIN32)
    set(CPACK_GENERATOR "NSIS" "ZIP")
    set(CPACK_NSIS_ENABLE_UNINSTALL_BEFORE_INSTALL ON)
    set(CPACK_NSIS_CREATE_ICONS_EXTRA "CreateDirectory \\\"$SMPROGRAMS\\\\$STARTMENU_FOLDER\\\"
    CreateShortCut \\\"$SMPROGRAMS\\\\$STARTMENU_FOLDER\\\\EasyCommandRunner.lnk\\\" \\\"$INSTDIR\\\\bin\\\\easy-command-runner.exe\\\"")
elseif(APPLE)
    set(CPACK_GENERATOR "DMG" "ZIP")
    set(CPACK_DMG_VOLUME_NAME "EasyCommandRunner")
    set(CPACK_DMG_DS_STORE_SETUP_SCRIPT "${CMAKE_CURRENT_SOURCE_DIR}/cmake/DMGSetup.scpt")
else()
    set(CPACK_GENERATOR "DEB" "RPM" "TGZ")
    set(CPACK_DEBIAN_PACKAGE_DEPENDS "libqt6core6, libqt6gui6, libqt6widgets6")
endif()

include(CPack)
