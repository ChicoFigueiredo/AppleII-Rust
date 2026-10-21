# Headless conformance oracle. Copied by build-audetto-oracle.sh to <fork>/source/frontends/oracle/
# and enabled with -DBUILD_ORACLE=ON (option added by audetto-trace.patch).

add_executable(audetto-oracle main.cpp audio.cpp)

target_include_directories(audetto-oracle PRIVATE
  ${CMAKE_BINARY_DIR}/source/frontends/common2
  )

target_link_libraries(audetto-oracle PRIVATE
  appleii
  common2
  ${NETWORK_LIBRARIES}
  )
