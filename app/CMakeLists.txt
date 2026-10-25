cmake_minimum_required(VERSION 3.21)
project(harbour-rootree VERSION 0.2.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick)

# il nucleo Rust lo compila tools/build.sh prima di cmake (cargo fuori da sb2)
set(RUST_LIB_DIR "" CACHE PATH "cartella di librootree_core.a e cxxbridge/")
if(NOT EXISTS "${RUST_LIB_DIR}/librootree_core.a")
    message(FATAL_ERROR "manca ${RUST_LIB_DIR}/librootree_core.a: lanciare tools/build.sh")
endif()

# niente qt_add_qml_module né risorse compilate per ora: sotto sb2 qmlcachegen
# e rcc lasciano file con permessi 000 (vedi RooTitanium/packaging/qt612); il
# QML si legge da file, accanto al binario
qt_add_executable(harbour-rootree
    src/main.cpp
    src/foldermodel.cpp src/foldermodel.h
    src/filesystem.cpp src/filesystem.h
    src/jobmanager.cpp src/jobmanager.h
    src/qtrust.h
)
target_include_directories(harbour-rootree PRIVATE "${RUST_LIB_DIR}/cxxbridge")
target_link_libraries(harbour-rootree PRIVATE
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick
    "${RUST_LIB_DIR}/librootree_core.a"
    # quelle che chiede la libreria statica di Rust (cargo --print native-static-libs)
    gcc_s util rt pthread m dl
)
