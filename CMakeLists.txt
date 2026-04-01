cmake_minimum_required(VERSION 3.24)

project(EasyCommandRunner LANGUAGES CXX)

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
    REQUIRED
)

# For now, we'll build just the C++ frontend without CXX-Qt integration
# The Rust backend will be integrated via FFI in a later phase
# To build Rust: cargo build [--release]

# Build Rust library first
add_custom_target(rust_build ALL
    COMMAND cargo build 
            --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
            $<IF:$<CONFIG:Release>,--release,>
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)

# Determine the Rust target library name and path
if(APPLE)
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/libeasy_command_runner.a")
elseif(WIN32)
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/easy_command_runner.lib")
else()
    set(RUST_LIB_PATH "${CMAKE_CURRENT_SOURCE_DIR}/target/$<IF:$<CONFIG:Release>,release,debug>/libeasy_command_runner.a")
endif()


# Source files
set(SOURCES
    cpp/app.cpp
)

set(HEADERS
    cpp/app.h
)

# Resources
set(RESOURCES
    resources/resources.qrc
)

# Create executable
add_executable(${PROJECT_NAME}
    ${SOURCES}
    ${HEADERS}
    ${RESOURCES}
)

# Link libraries
target_link_libraries(${PROJECT_NAME}
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Network
    Qt6::Sql
    Qt6::Concurrent
    ${RUST_LIB_PATH}
)

# Add dependency on Rust build
add_dependencies(${PROJECT_NAME} rust_build)

# Include directories
target_include_directories(${PROJECT_NAME}
    PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/cpp
    ${CMAKE_CURRENT_BINARY_DIR}
)

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
        MACOSX_BUNDLE_BUNDLE_VERSION "1.0.0"
        MACOSX_BUNDLE_SHORT_VERSION_STRING "1.0"
    )
endif()

# Install targets
install(TARGETS ${PROJECT_NAME}
    BUNDLE DESTINATION . COMPONENT Runtime
    RUNTIME DESTINATION bin COMPONENT Runtime
)

# CPack configuration
set(CPACK_PACKAGE_NAME "EasyCommandRunner")
set(CPACK_PACKAGE_VERSION "0.9.0")
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
