# AGENTS.md

이 문서는 `rit` 프로젝트를 구현하거나 수정하는 모든 AI 에이전트와 개발자가 반드시 먼저 읽어야 하는 최상위 작업 지침이다.

`rit`는 **Rust + Git**을 의미한다. 목표는 기존 Git 저장소와 최대한 호환되면서도, 내부 구조와 API를 훨씬 이해하기 쉽게 만든 새로운 Git 구현체를 작성하는 것이다.

`rit`는 단순한 Git wrapper가 아니다. `git` 실행 파일을 호출해서 결과를 포장하는 도구가 아니라, Git 저장소 포맷과 주요 동작을 Rust로 직접 구현하는 프로젝트이다.

단, 구현 중에는 반드시 기존 `git` 명령을 기준 구현(reference implementation)으로 사용하여 동작을 검증해야 한다.

---

## 1. 프로젝트의 핵심 목표

`rit`의 목표는 다음과 같다.

1. 기존 Git 저장소와 호환되는 Rust 기반 Git 구현체를 만든다.
2. 매우 쉬운 API를 제공한다.
3. 내부 코드는 코딩 입문자에서 중급자 수준의 개발자도 읽고 이해할 수 있게 작성한다.
4. 성능은 최대한 챙기되, 이해하기 어려운 미세 최적화보다 명확한 구조를 우선한다.
5. LFS, Xet, 인증, Sparse Checkout, Partial Clone, Semantic Diff, Virtual FS를 모듈로 제공한다.
6. 기본적으로 단일 실행 파일로 빌드할 수 있게 한다.
7. 설정 파일이 없어도 합리적인 기본값으로 동작한다.
8. 설정 파일이 있으면 고급 기능을 명시적으로 켤 수 있게 한다.
9. Git과 결과가 달라지는 경우에는 반드시 의도적인 차이인지 문서화한다.

`rit`는 "가장 빠른 Git"만을 목표로 하지 않는다. `rit`의 핵심 차별점은 **이해하기 쉬운 코드**, **간단한 API**, **모듈형 기능**, **안전한 저장소 조작**, **단일 바이너리 배포**이다.

---

## 2. 절대 원칙

다음 원칙은 항상 지켜야 한다.

### 2.1 Git 호환성이 최우선이다

외부에서 관찰 가능한 동작은 가능한 한 기존 Git과 같아야 한다.

비교 대상은 다음이다.

* stdout
* stderr
* exit code
* `.git` 내부 상태
* index 상태
* working tree 상태
* refs 상태
* object graph
* hooks 실행 여부
* 설정 해석 결과

`rit`의 내부 구현은 달라도 된다. 그러나 사용자가 관찰하는 결과가 달라지면 안 된다.

### 2.2 기존 Git 명령 목록과 문서는 실행 시점에 직접 확인한다

LLM의 내장 지식은 오래되었거나 틀릴 수 있다. Git 명령과 옵션을 구현할 때는 반드시 현재 환경의 Git을 직접 확인해야 한다.

Core 구현 또는 명령 구현을 시작하기 전에 반드시 다음 명령을 실행한다.

```bash
# 현재 기준 Git 버전을 확인한다.
git --version

# 현재 Git이 제공하는 모든 명령어 목록을 확인한다.
git help -a
```

특정 명령을 구현할 때는 반드시 다음 명령을 실행한다.

```bash
# 예: status 명령 구현 전
git help status

# 예: add 명령 구현 전
git help add

# 예: commit 명령 구현 전
git help commit
```

문서 확인 후에는 구현 노트에 다음을 남긴다.

* 확인한 Git 버전
* 확인한 명령어 이름
* 참고한 help 항목
* 구현한 옵션
* 아직 구현하지 않은 옵션
* 기존 Git과 의도적으로 다르게 처리한 부분

### 2.3 wrapper로 구현하지 않는다

`rit`는 내부 구현체이다. 다음 방식은 금지한다.

```rust
std::process::Command::new("git")
```

예외적으로 테스트, 호환성 비교, 기준 결과 생성에서는 기존 `git` 실행을 사용할 수 있다.

허용되는 예시는 다음과 같다.

```text
compatibility test에서 기존 git과 rit의 결과 비교
benchmark에서 기존 git과 rit의 성능 비교
문서 기반 동작 확인을 위한 기준 출력 수집
```

금지되는 예시는 다음과 같다.

```text
rit status 내부에서 git status 호출
rit add 내부에서 git add 호출
rit commit 내부에서 git commit 호출
rit clone 내부에서 git clone 호출
```

### 2.4 쉬운 코드가 우선이다

복잡한 트릭보다 읽기 쉬운 코드를 우선한다.

선호하는 방식은 다음이다.

* 함수 이름은 길어도 명확하게 작성한다.
* 타입 이름은 역할이 드러나게 작성한다.
* 작은 함수로 나눈다.
* 에러 메시지는 사람이 이해할 수 있게 작성한다.
* 주석은 "무엇"보다 "왜"를 설명한다.
* unsafe는 기본적으로 금지한다.
* 성능 최적화는 벤치마크와 함께 작성한다.

피해야 하는 방식은 다음이다.

* 과도한 매크로
* 의미가 불분명한 짧은 변수명
* 복잡한 lifetime 트릭
* 불필요한 unsafe
* 지나치게 일반화된 generic
* 성능 근거 없는 미세 최적화
* 한 함수에 여러 책임을 넣는 구현

---

## 3. 프로젝트 포지션

`rit`는 다음 위치를 목표로 한다.

```text
Git 호환 Rust 엔진
+ 쉬운 API
+ 단일 바이너리
+ 선택적 고급 모듈
+ 대용량 파일 친화
+ 모노레포 친화
+ IDE/CI 친화
```

`rit`는 `gitoxide`와 직접적으로 같은 포지션을 잡지 않는다.

`gitoxide`가 Rust 진영의 고성능 Git 구현체라면, `rit`는 다음에 집중한다.

* 매우 쉬운 API
* 읽기 쉬운 내부 구현
* 기능별 모듈화
* Git 기능의 제품 수준 추상화
* LFS/Xet/Sparse/Auth/Semantic Diff를 간단한 API로 제공
* 단일 실행 파일 배포

성능은 중요하다. 그러나 성능을 위해 코드 이해 가능성을 심하게 해치면 안 된다.

---

## 4. 빌드와 배포 목표

`rit`는 기본적으로 단일 실행 파일로 빌드되어야 한다.

기본 배포 형태는 다음과 같다.

```text
rit
```

선택 설정 파일이 있는 경우 다음과 같다.

```text
rit
rit.toml
```

`rit.toml`이 없으면 기본값으로 동작해야 한다.

### 4.1 단일 바이너리 원칙

가능하면 다음을 지향한다.

* 별도 런타임 설치 불필요
* 외부 `git` 바이너리 의존 없음
* 외부 `git-lfs` 바이너리 의존 없음
* CI 환경에 파일 하나만 복사해도 사용 가능
* Docker 이미지에서 쉽게 사용 가능
* Windows, macOS, Linux 지원

### 4.2 기능별 빌드

모든 기능을 항상 포함하지 않는다.

Cargo feature로 기능을 나눈다.

예시는 다음과 같다.

```toml
[features]
default = ["cli", "local", "http"]
full = [
    "cli",
    "local",
    "http",
    "ssh",
    "auth",
    "lfs",
    "xet",
    "sparse",
    "partial-clone",
    "semantic-diff",
    "vfs",
    "policy",
]
lfs = []
xet = []
auth = []
ssh = []
sparse = []
partial-clone = []
semantic-diff = []
vfs = []
policy = []
```

실제 feature 이름은 구현 상황에 맞게 조정할 수 있다. 다만 기능별 선택 가능성은 유지해야 한다.

### 4.3 권장 바이너리 종류

릴리스는 최소 두 종류를 고려한다.

```text
rit-min
  기본 Git 기능 중심의 작은 바이너리

rit-full
  LFS, Xet, Auth, Sparse, Semantic Diff, VFS 등을 포함한 전체 바이너리
```

단일 이름 `rit`만 배포하는 경우에도 빌드 feature를 통해 최소/전체 빌드를 만들 수 있어야 한다.

---

## 5. 설정 파일 원칙

설정 파일은 선택 사항이다.

기본 설정 파일 이름은 다음 중 하나를 사용한다.

```text
rit.toml
.rit.toml
```

저장소별 설정은 가능하면 `.git/config`와 충돌하지 않게 관리한다.

권장 설정 예시는 다음과 같다.

```toml
[core]
explain = false
safe_writes = true

[large_files]
mode = "auto"
default_backend = "auto"
max_regular_blob_size = "100 MiB"

[large_files.lfs]
enabled = true
patterns = ["*.zip", "*.mp4", "*.psd"]

[large_files.xet]
enabled = true
patterns = ["*.safetensors", "*.parquet", "*.bin"]

[auth]
mode = "auto"
use_system_keychain = true
use_ssh_agent = true

[workspace.mobile]
include = [
    "apps/mobile",
    "packages/ui",
    "tools/mobile-build",
]

[workspace.backend]
include = [
    "services/api",
    "packages/db",
    "tools/deploy",
]

[diff]
default_engine = "text"
semantic = false

[policy]
deny_secrets = true
protect_branches = ["main", "master"]
```

설정 파일이 없을 때 기본값은 보수적으로 동작해야 한다.

* 기존 Git 저장소를 손상시키지 않는다.
* LFS/Xet은 자동 감지할 수 있으나, 새 추적 규칙을 임의로 추가하지 않는다.
* 위험한 쓰기 작업은 명확한 사용자 요청이 있을 때만 수행한다.
* 알 수 없는 repository format은 쓰기 금지 또는 명확한 에러로 처리한다.

---

## 6. 권장 crate 구조

초기에는 workspace를 다음처럼 나눈다.

```text
crates/
  rit-cli/
    사용자 CLI 진입점

  rit-core/
    저장소 탐색, object model, refs, index, config, attributes, 기본 worktree 처리

  rit-object/
    blob, tree, commit, tag object 파싱과 직렬화

  rit-odb/
    loose object, packfile, pack index, object lookup

  rit-refs/
    HEAD, branch, tag, packed-refs, symbolic ref 처리

  rit-index/
    .git/index 읽기와 쓰기

  rit-worktree/
    working tree scan, ignore 처리, pathspec 처리

  rit-diff/
    text diff, binary diff, rename detection, semantic diff 공통 모델

  rit-transport/
    local, HTTP, SSH, Git protocol, fetch, push, clone

  rit-auth/
    credential helper, keychain, ssh-agent, token provider

  rit-large-files/
    대용량 파일 추상화 계층

  rit-lfs/
    Git LFS pointer, local object cache, batch API

  rit-xet/
    Xet backend, chunk, CAS, reconstruction, cache

  rit-sparse/
    sparse checkout, partial clone, workspace profile

  rit-vfs/
    lazy materialization, virtual working tree

  rit-policy/
    저장소 정책, 파일 크기 제한, secret 검사, branch 보호

  rit-testkit/
    기존 git과 rit의 결과 비교 도구
```

프로젝트가 너무 이른 시점에 과도하게 분리되어 복잡해지는 것은 피한다. 다만 crate 경계는 장기 구조를 염두에 두고 설계한다.

초기 구현에서는 `rit-core` 안에 단순하게 구현하고, 기능이 커지면 별도 crate로 분리해도 된다.

---

## 7. Core 구현 범위

Core는 작고 안정적이어야 한다.

Core가 담당하는 것은 다음이다.

* repository 발견
* `.git` 디렉터리 처리
* bare repository 처리
* config 읽기
* object hash 계산
* loose object 읽기/쓰기
* packfile 읽기
* refs 읽기/쓰기
* HEAD 처리
* index 읽기/쓰기
* working tree 상태 계산
* pathspec 처리
* attributes 읽기
* ignore 규칙 처리
* 기본 status/add/commit/log/diff에 필요한 공통 기능

Core가 직접 담당하지 않아야 하는 것은 다음이다.

* LFS 세부 구현
* Xet 세부 구현
* 인증 provider 세부 구현
* Semantic Diff 언어별 parser
* VFS 플랫폼별 세부 구현
* 특정 호스팅 서비스 전용 API

Core는 각 기능 모듈이 붙을 수 있는 trait와 명확한 데이터 모델을 제공한다.

---

## 8. 명령 구현 절차

새 명령을 구현할 때는 반드시 다음 순서를 따른다.

### 8.1 기준 문서 확인

먼저 현재 Git의 명령 목록과 대상 명령의 문서를 확인한다.

```bash
git --version
git help -a
git help <command>
```

예시는 다음과 같다.

```bash
git help status
git help add
git help commit
git help diff
git help log
git help clone
git help fetch
git help push
git help sparse-checkout
git help lfs
```

`git lfs`는 Git 기본 명령이 아니라 별도 확장일 수 있다. 시스템에 설치되어 있지 않을 수 있으므로, LFS 구현 시에는 `git-lfs` 설치 여부를 명확히 확인한다.

### 8.2 명령 범위 정의

각 명령 구현 문서에는 다음을 적는다.

```text
명령 이름:
기준 Git 버전:
구현 상태:
지원하는 옵션:
지원하지 않는 옵션:
Git과 동일해야 하는 출력:
의도적으로 다른 출력:
저장소를 변경하는지 여부:
위험한 작업 여부:
필요한 테스트:
```

### 8.3 호환성 테스트 작성

명령 구현 전 또는 구현과 동시에 기존 Git과 비교하는 테스트를 작성한다.

비교 항목은 다음이다.

```text
stdout
stderr
exit code
파일 내용
index 상태
refs 상태
object 개수
HEAD 위치
working tree 상태
```

### 8.4 최소 구현

처음부터 모든 옵션을 구현하지 않는다.

다음 순서로 구현한다.

1. 가장 흔한 기본 동작
2. porcelain 안정 출력
3. 주요 옵션
4. edge case
5. 성능 최적화
6. 고급 옵션

예를 들어 `status`는 다음 순서가 좋다.

```text
status --porcelain=v1
status 기본 출력
untracked 파일 처리
ignored 파일 처리
branch 정보
renames
submodule
sparse checkout 상태
```

---

## 9. 우선 구현 명령

초기 버전에서 우선 구현할 명령은 다음이다.

### 9.1 읽기 전용 명령

먼저 저장소를 변경하지 않는 명령부터 구현한다.

```text
rit version
rit help
rit rev-parse
rit cat-file
rit ls-tree
rit log
rit show
rit status --porcelain
rit diff --name-only
rit diff --stat
```

읽기 전용 명령을 먼저 구현하는 이유는 저장소 손상 위험이 낮기 때문이다.

### 9.2 로컬 쓰기 명령

그 다음 로컬 쓰기 명령을 구현한다.

```text
rit init
rit add
rit restore
rit reset
rit commit
rit checkout
rit branch
rit tag
```

### 9.3 네트워크 명령

그 다음 transport가 필요한 명령을 구현한다.

```text
rit clone
rit fetch
rit pull
rit push
```

### 9.4 고급 명령

그 다음 고급 기능을 구현한다.

```text
rit merge
rit rebase
rit cherry-pick
rit stash
rit worktree
rit sparse
rit lfs
rit xet
rit doctor
rit repair
```

---

## 10. API 설계 원칙

`rit`는 CLI뿐 아니라 Rust 라이브러리로도 쓰기 쉬워야 한다.

API는 다음처럼 읽혀야 한다.

```rust
let repo = Repository::open(".")?;
let status = repo.status().include_untracked(true).run()?;
```

좋은 API의 기준은 다음이다.

* 이름만 봐도 동작을 예측할 수 있다.
* Git 내부 용어를 몰라도 기본 기능을 쓸 수 있다.
* 고급 사용자는 Git 내부 모델에 접근할 수 있다.
* 결과는 문자열보다 구조화된 타입으로 제공한다.
* CLI 출력은 구조화된 결과를 별도 formatter가 변환한다.

나쁜 API의 예시는 다음이다.

```rust
repo.run_plumbing("update-index", args)?;
```

좋은 API의 예시는 다음이다.

```rust
repo.index()
    .add_path("src/main.rs")?
    .write()?;
```

### 10.1 쉬운 API와 고급 API를 모두 제공한다

쉬운 API 예시는 다음이다.

```rust
let repo = Repository::open(".")?;
repo.add("src/main.rs")?;
repo.commit("Update main file")?;
```

고급 API 예시는 다음이다.

```rust
let repo = Repository::open(".")?;
let mut index = repo.index().read()?;
index.add_path("src/main.rs")?;
index.write()?;

let tree_id = repo.write_tree_from_index(&index)?;
let commit_id = repo.commit_builder()
    .message("Update main file")
    .tree(tree_id)
    .parent_head()
    .create()?;
```

두 API는 같은 내부 기능을 사용해야 한다.

---

## 11. 에러 처리 원칙

에러는 명확해야 한다.

사용자는 다음을 알 수 있어야 한다.

* 무엇이 실패했는가
* 어떤 경로에서 실패했는가
* 어떤 Git object나 ref가 관련되었는가
* 복구 가능한가
* 사용자가 무엇을 해야 하는가

권장 에러 형태는 다음이다.

```rust
#[derive(Debug, thiserror::Error)]
pub enum RitError {
    #[error("repository not found from path: {path}")]
    RepositoryNotFound { path: PathBuf },

    #[error("object not found: {object_id}")]
    ObjectNotFound { object_id: ObjectId },

    #[error("unsupported repository format version: {version}")]
    UnsupportedRepositoryFormat { version: u32 },

    #[error("I/O error while reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}
```

`anyhow`는 CLI 최상위나 프로토타입에서는 사용할 수 있다. 라이브러리 crate의 public API에서는 가능한 한 명확한 error enum을 사용한다.

---

## 12. 저장소 쓰기 안전성

Git 저장소를 쓰는 코드는 항상 안전해야 한다.

다음 원칙을 지킨다.

* 임시 파일에 먼저 쓴다.
* fsync가 필요한 위치를 명확히 한다.
* 성공하면 atomic rename을 사용한다.
* 실패하면 기존 파일을 보존한다.
* lock file을 사용한다.
* lock 획득 실패 시 명확한 에러를 반환한다.
* 알 수 없는 repository format은 쓰지 않는다.
* 저장소 상태 변경 전후를 테스트한다.

권장 패턴은 다음이다.

```text
1. lock 획득
2. 현재 상태 읽기
3. 새 내용 임시 파일에 쓰기
4. flush/fsync
5. atomic rename
6. lock 해제
```

index, refs, config, packed-refs, LFS/Xet metadata는 특히 조심해서 쓴다.

---

## 13. 성능 원칙

성능은 중요하다. 그러나 성능 개선은 측정 가능해야 한다.

성능 최적화 전에는 다음을 확인한다.

* 실제 병목인가
* 대형 저장소에서 문제가 되는가
* 단순 구현으로 충분하지 않은가
* 개선 후 코드가 너무 어려워지지 않는가
* 벤치마크가 있는가

권장 최적화는 다음이다.

* 불필요한 파일 시스템 접근 줄이기
* object를 반복 파싱하지 않기
* streaming I/O 사용하기
* 큰 파일을 메모리에 통째로 올리지 않기
* pathspec matching 결과 캐싱하기
* ignore rule matching 비용 줄이기
* packfile index를 효율적으로 조회하기
* 대형 작업은 병렬화하되 결과 순서는 안정적으로 유지하기
* mmap은 필요할 때만 사용하고 fallback을 제공하기

금지하는 최적화는 다음이다.

* 벤치마크 없는 복잡한 최적화
* unsafe 기반 미세 최적화
* 이해하기 어려운 매크로 최적화
* 플랫폼별 동작을 깨뜨리는 최적화
* Git 호환성을 깨뜨리는 최적화

---

## 14. 코드 스타일

Rust 코드는 다음을 따른다.

* stable Rust를 우선한다.
* `cargo fmt`를 통과해야 한다.
* `cargo clippy` 경고를 가능한 한 해결한다.
* public item에는 문서 주석을 작성한다.
* 테스트하기 쉬운 작은 단위로 작성한다.
* 함수 하나는 한 가지 책임만 가진다.
* I/O와 순수 로직을 분리한다.
* CLI 출력과 core 로직을 분리한다.
* platform-specific 코드는 별도 모듈로 격리한다.

변수명은 명확해야 한다.

좋은 예시는 다음이다.

```rust
let repository_path = find_repository_root(current_dir)?;
let object_id = hash_blob(contents);
let index_entries = index.read_entries()?;
```

피해야 하는 예시는 다음이다.

```rust
let p = f(c)?;
let x = h(b);
let v = i.r()?;
```

짧은 변수명은 아주 좁은 범위에서만 허용한다.

---

## 15. 주석 원칙

코드는 읽기 쉬워야 한다. 주석은 복잡한 Git 동작을 설명하는 데 사용한다.

좋은 주석은 다음을 설명한다.

* 왜 이 순서로 처리하는가
* Git과 호환하려면 왜 이 edge case가 필요한가
* 이 코드가 저장소 손상을 어떻게 방지하는가
* 성능상 왜 이 캐시가 필요한가

나쁜 주석은 다음이다.

```rust
// i를 1 증가시킨다.
i += 1;
```

좋은 주석은 다음이다.

```rust
// Git index는 파일의 stat 정보가 그대로이면 content hash 계산을 생략할 수 있다.
// 이 최적화는 대형 저장소에서 status 성능에 큰 영향을 준다.
if entry.stat_matches(file_stat) {
    return Ok(FileStatus::Clean);
}
```

---

## 16. 테스트 전략

테스트는 다음 계층으로 작성한다.

### 16.1 단위 테스트

순수 함수와 parser를 테스트한다.

예시는 다음이다.

* object header parser
* commit parser
* tree parser
* index parser
* refname validator
* pathspec matcher
* ignore matcher
* LFS pointer parser
* Xet pointer/parser

### 16.2 통합 테스트

임시 저장소를 만들고 `rit` 기능을 실행한다.

예시는 다음이다.

* init 후 `.git` 구조 확인
* add 후 index 확인
* commit 후 object graph 확인
* status 결과 확인
* diff 결과 확인

### 16.3 Git 호환성 테스트

같은 저장소에서 기존 `git`과 `rit`를 모두 실행하고 결과를 비교한다.

테스트 도구는 다음을 비교해야 한다.

```text
stdout
stderr
exit code
working tree
index
refs
object ids
```

가능하면 porcelain 출력부터 비교한다.

예시는 다음이다.

```bash
git status --porcelain=v1
rit status --porcelain=v1
```

### 16.4 대형 저장소 테스트

대형 저장소 테스트는 별도로 둔다.

테스트 항목은 다음이다.

* 파일 수가 매우 많은 저장소
* 큰 blob이 많은 저장소
* deep history 저장소
* 많은 branch와 tag
* 많은 ignored file
* sparse checkout 저장소
* partial clone 저장소
* LFS 저장소
* Xet 저장소

### 16.5 손상 저장소 테스트

실제 환경에서는 저장소가 깨질 수 있다.

다음을 테스트한다.

* 누락된 object
* 잘못된 ref
* 깨진 index
* 깨진 packfile
* 잘못된 config
* 권한 없는 파일
* 중간에 실패한 lock file

`rit`는 이런 상황에서 panic하지 않고 명확한 에러를 내야 한다.

---

## 17. 호환성 테스트 하네스

`rit-testkit`은 기존 Git과 `rit`를 비교하는 도구를 제공한다.

권장 API는 다음이다.

```rust
CompatibilityTest::new()
    .given_repo("basic-commit")
    .run_git(["status", "--porcelain=v1"])
    .run_rit(["status", "--porcelain=v1"])
    .expect_same_stdout()
    .expect_same_exit_code()
    .run()?;
```

CLI 형태도 제공할 수 있다.

```bash
rit-testkit compare -- git status --porcelain=v1 -- rit status --porcelain=v1
```

테스트 결과는 사람이 읽기 쉬워야 한다.

```text
Command: status --porcelain=v1
Git exit code: 0
Rit exit code: 0
Stdout: same
Stderr: different
First difference:
  line 3
  git: warning: ...
  rit: warning: ...
```

---

## 18. Large Object Backend

LFS와 Xet은 별도 기능이지만, 사용자 관점에서는 모두 대용량 파일 처리 기능이다.

따라서 공통 추상화를 둔다.

```text
Large Object Backend
  LFS
  Xet
  local CAS
  S3-compatible storage
  custom backend
```

권장 API는 다음이다.

```rust
let repo = Repository::open(".")?;

repo.large_files()
    .backend(LargeFileBackend::Auto)
    .track("*.safetensors")
    .track("*.parquet")
    .apply()
    .await?;
```

자동 선택 규칙은 보수적으로 설계한다.

예시는 다음이다.

```text
GitHub/GitLab 일반 저장소: LFS 우선
Hugging Face 저장소: Xet 감지 시 Xet 우선
명시 설정 존재: 명시 설정 우선
알 수 없는 원격: 기존 Git/LFS 호환 동작 우선
```

자동 선택은 사용자의 저장소를 임의로 변경하면 안 된다.

---

## 19. LFS 모듈

LFS는 선택적 모듈이다.

구현 목표는 다음이다.

* LFS pointer file 파싱
* LFS pointer file 생성
* local LFS object cache
* SHA-256 검증
* size 검증
* Batch API download
* Batch API upload
* checkout materialization
* add 시 clean 동작
* push 전 upload 확인

LFS 구현 원칙은 다음이다.

* 전체 파일을 메모리에 올리지 않는다.
* 항상 streaming으로 처리한다.
* 다운로드한 object는 SHA-256과 size를 검증한 뒤 사용한다.
* 검증 실패 object는 working tree에 쓰지 않는다.
* 외부 `git-lfs` 실행 파일에 의존하지 않는다.
* 기존 LFS pointer format과 호환되어야 한다.

---

## 20. Xet 모듈

Xet은 선택적 모듈이다.

Xet은 Hugging Face와 AI/ML 대용량 저장소 사용자를 위해 중요하다.

구현 목표는 다음이다.

* Xet 기반 저장소 감지
* chunk 기반 저장소 모델
* CAS object 처리
* local chunk cache
* file reconstruction
* deduplication 친화 구조
* Hugging Face 저장소 인증 연동
* LFS와 충돌하지 않는 대용량 파일 backend 선택

초기에는 완전한 자체 구현보다 호환 가능한 얇은 모듈부터 시작할 수 있다.

장기적으로는 다음을 지향한다.

```text
rit clone https://huggingface.co/org/model
  원격이 Xet을 사용하면 자동 감지
  필요한 파일만 다운로드
  큰 모델 파일은 chunk cache 활용
  인증은 auth 모듈과 연동
```

Xet 기능이 비활성화된 빌드에서는 명확한 에러를 반환한다.

```text
This repository uses Xet storage, but this rit build does not include Xet support.
Please install rit-full or rebuild with the `xet` feature.
```

---

## 21. Auth & Credentials

인증은 독립 모듈이어야 한다.

목표는 사용자가 인증 방식을 자세히 몰라도 동작하게 하는 것이다.

지원 대상은 다음이다.

* HTTPS token
* Git credential helper 호환
* OS keychain
* SSH key
* ssh-agent
* environment variable token
* CI token
* Hugging Face token
* GitHub token
* GitLab token
* OAuth device flow

권장 API는 다음이다.

```rust
let repo = Repository::clone("https://github.com/example/project.git")
    .auth(Auth::auto())
    .run()
    .await?;
```

보안 원칙은 다음이다.

* token을 로그에 출력하지 않는다.
* 에러 메시지에 secret을 포함하지 않는다.
* debug 출력에서도 secret을 마스킹한다.
* credential 저장은 사용자가 허용한 경우에만 수행한다.
* CI에서는 interactive prompt를 기본적으로 사용하지 않는다.

---

## 22. Sparse Checkout, Partial Clone, Workspace

`rit`는 모노레포 사용성을 중요하게 다룬다.

Sparse Checkout과 Partial Clone은 내부 구현 용어로만 두지 않는다. 사용자에게는 Workspace 개념으로 제공한다.

권장 API는 다음이다.

```rust
let repo = Repository::clone("https://example.com/huge/mono.git")
    .workspace("mobile")
    .include("apps/mobile")
    .include("packages/ui")
    .partial_clone(true)
    .lazy_files(true)
    .run()
    .await?;
```

CLI 예시는 다음이다.

```bash
rit clone https://example.com/huge/mono.git --workspace mobile
rit workspace use mobile
rit workspace include apps/mobile packages/ui
rit workspace prefetch
```

구현 원칙은 다음이다.

* 사용자는 sparse checkout 내부 규칙을 몰라도 되어야 한다.
* 필요한 파일만 checkout할 수 있어야 한다.
* 필요한 blob만 나중에 받을 수 있어야 한다.
* 네트워크 지연을 줄이기 위해 prefetch 정책을 제공한다.
* 기존 Git sparse checkout 저장소를 읽을 수 있어야 한다.

---

## 23. Virtual FS

VFS는 선택적 고급 기능이다.

목표는 매우 큰 저장소에서 파일을 실제로 모두 내려받지 않고도 작업할 수 있게 하는 것이다.

구현 목표는 다음이다.

* lazy materialization
* 파일 접근 시 blob 다운로드
* background prefetch
* local blob cache
* sparse workspace와 연동
* 플랫폼별 backend 분리

VFS는 플랫폼 의존성이 크므로 core에 넣지 않는다.

권장 구조는 다음이다.

```text
rit-vfs
  common model
  windows backend
  macos backend
  linux backend
  fallback materialized backend
```

VFS가 없는 환경에서도 `rit`는 동작해야 한다.

---

## 24. Semantic Diff

Semantic Diff는 `rit`의 중요한 차별점이다.

목표는 단순 줄 단위 diff가 아니라 코드 구조를 이해하는 diff를 제공하는 것이다.

초기 구현은 다음 순서가 좋다.

1. 일반 text diff
2. word diff
3. rename detection
4. tree-sitter 기반 syntax tree diff
5. 언어별 semantic summary
6. public API 변경 감지

권장 API는 다음이다.

```rust
let diff = repo.diff()
    .between("main", "HEAD")
    .semantic()
    .language(Language::Auto)
    .run()
    .await?;
```

Semantic Diff 출력은 다음처럼 구조화되어야 한다.

```text
함수 추가
함수 삭제
함수 이름 변경
함수 본문 변경
public API 변경
import 정리
테스트만 변경
타입 정의 변경
파일 이동
```

Semantic Diff는 언어별 plugin 또는 adapter 구조로 구현한다.

```text
rit-diff
  기본 diff 모델

rit-semantic-diff
  semantic diff 공통 모델

rit-semantic-rust
  Rust parser adapter

rit-semantic-typescript
  TypeScript parser adapter

rit-semantic-python
  Python parser adapter
```

초기에는 tree-sitter를 사용하는 것이 좋다. 다만 tree-sitter 의존성은 feature로 분리한다.

---

## 25. Explainable Git

`rit`는 이해하기 쉬운 Git 구현체를 지향한다.

따라서 가능한 경우 "왜 이런 결과가 나왔는지" 설명할 수 있어야 한다.

예시는 다음이다.

```bash
rit status --explain src/main.rs
```

출력 예시는 다음이다.

```text
src/main.rs is modified.
Reason:
  - The file exists in HEAD.
  - The file exists in the index.
  - The index stat information does not match the working tree file.
  - rit recalculated the file content hash.
  - The new blob hash is different from the index blob hash.
```

API 예시는 다음이다.

```rust
let explanation = repo.status()
    .explain("src/main.rs")
    .await?;
```

Explain 기능은 디버깅, 교육, IDE 통합에 유용하다.

---

## 26. Policy Engine

정책 엔진은 선택적 모듈이다.

목표는 팀이나 회사가 저장소 규칙을 쉽게 정의하게 하는 것이다.

지원할 수 있는 정책은 다음이다.

* 일반 Git blob 최대 크기 제한
* 특정 확장자는 LFS 사용 강제
* 특정 확장자는 Xet 사용 강제
* secret 패턴 commit 차단
* 보호 branch force push 차단
* 특정 경로 변경 시 semantic diff 요구
* 특정 경로 변경 시 reviewer 요구 정보 생성
* binary file 직접 commit 경고

권장 API는 다음이다.

```rust
repo.policy()
    .max_regular_blob_size("100 MiB")
    .use_lfs_for(["*.mp4", "*.zip"])
    .use_xet_for(["*.safetensors", "*.parquet"])
    .deny_secrets(true)
    .protect_branch("main")
    .apply()?;
```

정책 엔진은 기본적으로 경고부터 제공하고, 명시 설정이 있을 때 차단한다.

---

## 27. CLI 설계

`rit` CLI는 두 가지 목표를 가진다.

1. 기존 Git 사용자에게 익숙한 명령을 제공한다.
2. 새 사용자에게 더 쉬운 명령을 제공한다.

기존 호환 명령 예시는 다음이다.

```bash
rit status
rit add .
rit commit -m "message"
rit log
rit diff
rit clone <url>
```

쉬운 명령 예시는 다음이다.

```bash
rit save "message"
rit undo
rit workspace use mobile
rit large-files track "*.zip"
rit doctor
```

CLI 출력은 core 로직에 직접 섞지 않는다.

구조는 다음을 따른다.

```text
core function returns structured data
formatter converts structured data to human output
json formatter converts structured data to JSON
compat formatter matches Git output when required
```

JSON 출력은 IDE와 CI를 위해 제공한다.

```bash
rit status --format json
rit diff --semantic --format json
rit policy check --format json
```

---

## 28. JSON과 Typed Output

CLI 출력 파싱은 취약하다. 따라서 `rit`는 구조화된 출력을 중요하게 다룬다.

지원 포맷은 다음을 고려한다.

```text
human
json
porcelain
```

예시는 다음이다.

```bash
rit status --format json
```

출력 예시는 다음이다.

```json
{
  "branch": "main",
  "changes": [
    {
      "path": "src/main.rs",
      "status": "modified",
      "staged": false,
      "working_tree": true
    }
  ]
}
```

JSON schema는 안정적으로 관리한다.

---

## 29. Path 처리 원칙

Git 구현에서 path 처리는 매우 중요하다.

다음 원칙을 지킨다.

* 내부 Git path는 `/` 구분자를 사용한다.
* OS path와 Git path를 구분하는 타입을 둔다.
* Windows path 처리를 별도로 테스트한다.
* Unicode normalization 문제를 고려한다.
* case-insensitive filesystem을 고려한다.
* symlink 처리 차이를 테스트한다.
* file mode 차이를 테스트한다.

권장 타입은 다음이다.

```rust
pub struct GitPath(String);
pub struct WorktreePath(PathBuf);
```

문자열을 무분별하게 path로 사용하지 않는다.

---

## 30. Hash와 Object ID

초기에는 SHA-1 Git 저장소를 우선 지원한다.

장기적으로 SHA-256 저장소도 지원할 수 있게 설계한다.

따라서 object id를 단순 `[u8; 20]`으로 고정하지 않는다.

권장 모델은 다음이다.

```rust
pub enum ObjectHashKind {
    Sha1,
    Sha256,
}

pub struct ObjectId {
    kind: ObjectHashKind,
    bytes: Vec<u8>,
}
```

성능상 필요하면 내부 최적화 타입을 추가할 수 있다. 그러나 public API는 hash algorithm 확장을 고려해야 한다.

---

## 31. Packfile 구현 원칙

Packfile은 Git 성능과 호환성의 핵심이다.

초기 구현 목표는 다음이다.

* pack index 읽기
* object offset lookup
* packed object header 파싱
* delta object 해석
* base object 복원
* object checksum 검증

원칙은 다음이다.

* packfile 전체를 불필요하게 메모리에 올리지 않는다.
* streaming과 random access를 모두 고려한다.
* delta resolution cache를 둔다.
* 깨진 packfile은 panic하지 않고 에러 처리한다.
* packfile writer는 reader보다 나중에 구현한다.

---

## 32. Index 구현 원칙

`.git/index`는 status/add/commit의 핵심이다.

구현 목표는 다음이다.

* index header 파싱
* index entry 파싱
* file mode 처리
* stat 정보 처리
* stage 처리
* conflict entry 처리
* index extension 처리
* index 쓰기
* lock 기반 atomic update

처음부터 모든 extension을 완벽히 구현하지 못해도 된다. 하지만 알 수 없는 extension을 발견했을 때 저장소를 손상시키면 안 된다.

원칙은 다음이다.

```text
읽을 수 없는 index는 명확한 에러
보존해야 할 extension은 보존
이해하지 못한 정보를 삭제하지 않기
쓰기 전 백업 또는 lock 사용
```

---

## 33. Refs 구현 원칙

Refs는 branch, tag, HEAD의 핵심이다.

구현 목표는 다음이다.

* loose refs 읽기
* loose refs 쓰기
* symbolic ref 처리
* HEAD 처리
* packed-refs 읽기
* tag 처리
* ref lock 처리
* atomic ref update

원칙은 다음이다.

* ref update는 기대한 old value와 비교하여 안전하게 수행한다.
* 동시에 다른 프로세스가 ref를 바꿀 수 있음을 고려한다.
* refname validation을 구현한다.
* packed-refs 수정은 특히 조심한다.

---

## 34. Worktree와 Ignore 처리

Working tree scan은 status 성능의 핵심이다.

구현 목표는 다음이다.

* tracked file 상태 계산
* untracked file 탐색
* ignored file 처리
* `.gitignore` 처리
* `.git/info/exclude` 처리
* global excludes 처리
* pathspec 처리
* file mode 변경 처리
* symlink 처리

성능 원칙은 다음이다.

* 디렉터리 스캔을 최소화한다.
* ignore rule을 반복 파싱하지 않는다.
* pathspec으로 탐색 범위를 줄인다.
* 필요하지 않으면 파일 content hash를 다시 계산하지 않는다.
* stat 정보로 빠르게 clean 여부를 판단한다.

---

## 35. Transport 구현 원칙

Transport는 clone/fetch/push의 핵심이다.

초기 지원 순서는 다음이 좋다.

1. local repository transport
2. HTTP read-only fetch
3. HTTP fetch/push
4. SSH fetch/push
5. protocol v2 고급 기능

Transport는 auth 모듈과 분리한다.

원칙은 다음이다.

* credential은 transport 내부에 직접 저장하지 않는다.
* retry 정책은 명확해야 한다.
* partial clone filter를 지원할 수 있게 설계한다.
* progress 출력과 데이터 전송을 분리한다.
* 네트워크 에러는 복구 가능성을 알려준다.

---

## 36. Hooks

기존 Git hook과의 호환성을 고려해야 한다.

초기에는 hook 실행을 제한적으로 지원할 수 있다.

지원 후보는 다음이다.

```text
pre-commit
commit-msg
pre-push
post-checkout
post-merge
```

원칙은 다음이다.

* hook 실행 여부를 명확히 한다.
* hook stdout/stderr 처리를 Git과 비교한다.
* hook 실패 시 exit code를 정확히 처리한다.
* 보안상 hook 자동 실행은 민감할 수 있으므로 문서화한다.

---

## 37. Submodule

Submodule은 복잡하므로 초기 core 범위에서는 최소 지원으로 시작한다.

초기 목표는 다음이다.

* `.gitmodules` 읽기
* status에서 submodule 존재 인식
* submodule entry 표시

나중 목표는 다음이다.

* submodule init
* submodule update
* nested submodule
* sparse workspace와 submodule 연동

Submodule은 많은 edge case가 있으므로 기존 Git과 철저히 비교한다.

---

## 38. Branch, Merge, Rebase

Merge와 Rebase는 저장소를 크게 변경하므로 뒤로 미룬다.

초기에는 다음을 먼저 구현한다.

* branch 목록
* branch 생성
* branch 삭제
* checkout/switch 기본 동작
* fast-forward merge

그 다음 다음을 구현한다.

* three-way merge
* conflict marker 생성
* merge state 저장
* merge abort
* cherry-pick
* rebase

원칙은 다음이다.

* conflict 처리 결과를 기존 Git과 비교한다.
* merge state 파일을 정확히 관리한다.
* 실패 시 복구 가능해야 한다.

---

## 39. Doctor와 Repair

`rit doctor`는 중요한 차별점이다.

목표는 저장소 상태를 진단하고 사용자가 이해할 수 있게 설명하는 것이다.

검사 항목은 다음이다.

* HEAD 유효성
* refs 유효성
* object 존재 여부
* index 파싱 가능 여부
* packfile 유효성
* LFS object 누락
* Xet chunk 누락
* config 문제
* lock file 잔재
* working tree 권한 문제

`rit repair`는 매우 보수적으로 구현한다.

* 자동 복구 가능한 항목만 복구한다.
* 위험한 복구는 사용자 확인이 필요하다.
* 복구 전 백업을 만든다.
* 수행한 작업을 로그로 남긴다.

---

## 40. 보안 원칙

`rit`는 저장소, 인증 정보, hook, 네트워크를 다루므로 보안이 중요하다.

원칙은 다음이다.

* token과 password를 로그에 남기지 않는다.
* path traversal을 방지한다.
* archive/checkout 시 working tree 밖으로 파일을 쓰지 않는다.
* symlink 공격을 고려한다.
* hook 실행은 명확히 통제한다.
* 원격 URL을 출력할 때 credential을 마스킹한다.
* 외부 프로세스 실행은 최소화한다.
* 임시 파일 권한을 안전하게 설정한다.

---

## 41. 로깅과 추적

로깅은 문제 해결에 유용해야 하지만, 사용자를 방해하면 안 된다.

권장 로그 레벨은 다음이다.

```text
error
warn
info
debug
trace
```

민감 정보는 항상 마스킹한다.

성능 분석을 위해 선택적으로 trace를 제공한다.

```bash
RIT_TRACE=1 rit status
RIT_TRACE_PERF=1 rit status
```

출력 예시는 다음과 같다.

```text
scan_worktree: 18ms
read_index: 4ms
match_ignore_rules: 6ms
compute_status: 11ms
```

---

## 42. 문서화 원칙

모든 주요 모듈은 문서가 있어야 한다.

문서에는 다음을 포함한다.

* 이 모듈의 책임
* 이 모듈이 책임지지 않는 것
* 주요 타입
* 주요 흐름
* Git 호환성 주의점
* 테스트 방법
* 성능 주의점

문서 위치 예시는 다음이다.

```text
docs/core.md
docs/index.md
docs/objects.md
docs/refs.md
docs/worktree.md
docs/transport.md
docs/lfs.md
docs/xet.md
docs/sparse.md
docs/semantic-diff.md
docs/auth.md
docs/config.md
docs/compatibility.md
```

---

## 43. 작업 시작 전 체크리스트

AI 에이전트는 작업 시작 전 다음을 확인한다.

```text
1. 어떤 명령 또는 모듈을 구현하는가?
2. 이 작업이 저장소를 변경하는가?
3. 기준 Git 명령 문서를 확인했는가?
4. git help -a를 확인했는가?
5. git help <command>를 확인했는가?
6. 기존 Git과 비교할 테스트를 만들었는가?
7. 읽기 쉬운 구조로 설계했는가?
8. 에러 처리를 명확히 했는가?
9. 저장소 손상 가능성을 검토했는가?
10. 성능 측정 방법을 정했는가?
```

---

## 44. 구현 완료 전 체크리스트

AI 에이전트는 작업 완료 전 다음을 확인한다.

```text
1. cargo fmt를 통과했는가?
2. cargo clippy를 통과했는가?
3. cargo test를 통과했는가?
4. 기존 Git과 호환성 테스트를 수행했는가?
5. 새 기능의 문서를 작성했는가?
6. public API에 문서 주석을 작성했는가?
7. 에러 메시지가 명확한가?
8. panic 가능성이 없는가?
9. unwrap/expect가 불필요하게 사용되지 않았는가?
10. 저장소 쓰기 작업이 lock/atomic write를 사용하는가?
11. 대형 파일을 메모리에 통째로 올리지 않는가?
12. token이나 secret이 로그에 노출되지 않는가?
13. 기능이 feature flag로 분리되어야 한다면 분리했는가?
```

---

## 45. 권장 초기 마일스톤

### Milestone 0: 프로젝트 뼈대

* Cargo workspace 생성
* `rit` CLI 생성
* `rit --version`
* `rit help`
* 기본 에러 타입
* 기본 logging
* 테스트 디렉터리 구조

### Milestone 1: 읽기 전용 core

* repository 발견
* `.git` 디렉터리 탐색
* config 읽기
* loose object 읽기
* commit/tree/blob parser
* `rit cat-file`
* `rit ls-tree`
* `rit log` 기본

### Milestone 2: index와 status

* index 읽기
* working tree scan
* ignore 처리
* pathspec 처리
* `rit status --porcelain=v1`
* 기존 Git과 status 비교 테스트

### Milestone 3: 로컬 쓰기

* loose object 쓰기
* index 쓰기
* `rit init`
* `rit add`
* `rit commit`
* refs update
* atomic write

### Milestone 4: diff

* text diff
* name-only diff
* stat diff
* binary file 감지
* rename detection 초기 버전

### Milestone 5: transport와 auth

* local clone
* HTTP fetch
* 기본 credential 처리
* GitHub/GitLab/Hugging Face remote 감지

### Milestone 6: Large files

* Large Object Backend 추상화
* LFS pointer parser
* LFS local cache
* Xet 감지
* Xet feature gate

### Milestone 7: monorepo 기능

* sparse workspace
* partial clone 기초
* workspace profile
* prefetch 정책

### Milestone 8: semantic diff

* semantic diff 공통 모델
* tree-sitter adapter
* Rust/TypeScript/Python 중 하나 우선 지원
* JSON 출력

### Milestone 9: VFS와 고급 기능

* lazy materialization
* blob cache
* platform backend
* doctor/repair
* policy engine

---

## 46. 의존성 선택 원칙

의존성은 신중하게 추가한다.

허용 기준은 다음이다.

* 널리 사용되는 crate인가?
* 유지보수가 되고 있는가?
* 보안 이슈가 적은가?
* 바이너리 크기에 미치는 영향이 적절한가?
* feature flag로 분리할 수 있는가?
* 직접 구현하는 것보다 명확히 나은가?

권장 성격의 의존성은 다음이다.

```text
clap 또는 유사 CLI parser
thiserror
tracing
serde
serde_json
toml
sha1 / sha2 관련 crate
flate2 또는 zlib backend
reqwest 또는 hyper 계열 HTTP client
tokio는 async transport가 필요할 때 선택
```

주의할 의존성은 다음이다.

```text
무거운 runtime
불필요하게 큰 framework
유지보수 중단 crate
unsafe가 많은 crate
플랫폼 지원이 약한 crate
```

---

## 47. 비동기 처리 원칙

네트워크, LFS, Xet, VFS, prefetch는 async가 유용할 수 있다.

하지만 core object parsing, index parsing, refs 처리 같은 로컬 순수 로직은 동기 코드가 더 이해하기 쉬울 수 있다.

원칙은 다음이다.

```text
core는 가능한 단순한 sync API 우선
transport와 대용량 파일 전송은 async 허용
sync wrapper와 async API의 경계를 명확히 함
```

초기 구현에서 async가 전체 코드 복잡도를 크게 올리면, 명확한 blocking 구현으로 시작해도 된다.

---

## 48. Panic 금지 원칙

라이브러리 코드는 일반 입력이나 손상된 저장소 때문에 panic하면 안 된다.

금지 예시는 다음이다.

```rust
let header = parse_header(bytes).unwrap();
```

권장 예시는 다음이다.

```rust
let header = parse_header(bytes)?;
```

`unwrap`과 `expect`는 다음 경우에만 제한적으로 허용한다.

* 테스트 코드
* 명백한 내부 불변식 검증
* 프로그램 시작 시 정적 값 검증

사용할 경우 왜 안전한지 주석을 남긴다.

---

## 49. Git과 다르게 할 수 있는 부분

기본은 Git 호환이다. 그러나 다음은 명시적으로 다르게 설계할 수 있다.

* 더 친절한 에러 메시지
* 추가 JSON 출력
* `--explain` 출력
* 쉬운 별칭 명령
* workspace profile
* policy warning
* large file 자동 경고
* semantic diff summary
* doctor/repair 진단

다르게 동작하는 부분은 반드시 문서화한다.

---

## 50. AI 에이전트 응답 원칙

AI 에이전트가 이 프로젝트에서 작업할 때는 다음 방식으로 응답한다.

1. 먼저 현재 작업 범위를 짧게 요약한다.
2. 관련 Git 명령이면 `git help -a`와 `git help <command>` 확인 여부를 밝힌다.
3. 구현할 파일과 테스트할 파일을 말한다.
4. 변경은 작은 단위로 나눈다.
5. 모든 코드는 생략 없이 작성한다.
6. 테스트 방법을 구체적으로 적는다.
7. 구현하지 않은 옵션이나 제한 사항을 솔직히 밝힌다.

---

## 51. 최종 제품 이미지

`rit`는 다음처럼 사용할 수 있어야 한다.

```bash
# 일반 Git처럼 사용
rit status
rit add .
rit commit -m "Update project"
rit log
rit diff

# 큰 저장소를 쉽게 clone
rit clone https://example.com/huge/mono.git --workspace mobile

# 대용량 파일 자동 처리
rit large-files track "*.safetensors" --backend xet
rit large-files track "*.zip" --backend lfs

# 코드 구조 기반 diff
rit diff --semantic main..HEAD

# 저장소 문제 진단
rit doctor

# CI용 구조화 출력
rit status --format json
rit diff --semantic --format json
```

Rust API는 다음처럼 사용할 수 있어야 한다.

```rust
let repo = Repository::clone("https://example.com/huge/mono.git")
    .auth(Auth::auto())
    .workspace("mobile")
    .large_files(LargeFiles::auto())
    .run()
    .await?;

let changes = repo.status()
    .include_untracked(true)
    .typed()
    .await?;

let diff = repo.diff()
    .between("main", "HEAD")
    .semantic()
    .run()
    .await?;
```

---

## 52. 한 문장 요약

`rit`는 기존 Git 저장소와 호환되면서도, Rust로 작성된 읽기 쉬운 내부 구조와 매우 쉬운 API를 제공하는 단일 바이너리 Git 엔진이다. LFS, Xet, Auth, Sparse Checkout, Partial Clone, Semantic Diff, VFS를 선택적 모듈로 제공하여 대형 저장소와 현대 개발 환경을 더 쉽게 다룰 수 있게 한다.
