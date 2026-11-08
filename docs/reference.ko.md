[쉬운 시작 안내로 돌아가기](../README.ko.md)

# N2 Sindri

Rust·TypeScript/TSX·JavaScript/JSX·Python·Go·Java 소스의 AST 기반 품질 검사기입니다. Rust 라이브러리와 JSON CLI를 제공합니다.

## 설치

Rust 1.98 이상과 C 빌드 도구가 필요합니다. Windows에서는 Visual Studio C++ Build Tools를 설치하세요.

```sh
cargo install --git https://github.com/choihyunsus/n2-sindri --tag v1.0.0 --locked
```

## 실행

```sh
n2-sindri --dir ./src
n2-sindri --files "src/main.rs,src/lib.rs"
```

결과는 JSON으로 출력합니다. 종료 코드 0은 품질 게이트 통과, 1은 검사 실패, 2는 잘못된 실행 인자 또는 출력 직렬화 오류입니다. 결과의 `passed`, `blockers`, `warnings`, `info`, `scanned_files`, `report` 필드는 대붕이 원본과 같습니다.

문법 오류, 일부 위험 구문, 파일·함수 길이와 반복 숫자 등을 검사합니다. 기능의 실제 작동 여부는 실행 테스트로 확인해야 합니다. 최대 5,000개 파일과 파일당 2 MiB를 처리합니다. 디렉터리 탐색에서는 심볼릭 링크와 의존성·빌드 폴더를 건너뜁니다. 검사할 소스가 없거나 파일을 읽을 수 없으면 실패합니다.

CLI는 사용자가 지정한 경로를 읽습니다. AI 도구로 연결하는 호스트는 자신의 작업공간과 권한 정책을 적용해야 합니다.

## Soul 연동

검사 자체는 DB 없이 실행합니다. 결과 저장은 호스트에서 Soul 9.2의 `n2_kv_save` 도구를 호출해 처리합니다. `agent`, `project`, `summary`, `decisions`, `todo` 필드를 사용하며 실제 검사 결과를 요약에 넣고 저장 응답을 확인하세요. 영문 README에 호출 인자 예제가 있습니다.

## 개발 검증

`cargo test --locked`와 `cargo package --locked`로 테스트와 배포본 빌드를 확인합니다. MIT 라이선스와 원본 저작권 고지를 포함합니다.
