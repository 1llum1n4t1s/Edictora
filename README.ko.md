<p align="center">
  <img src="icon/app_icon.png" alt="Edictora 아이콘" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | 한국어 | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**각 확장 프로그램의 저장소에 보관한 원본 파일을 바탕으로, Chrome Web Store 등록 정보(설명과 URL)를 일괄적으로 읽고, 비교하고, 업데이트하는 Windows용 도구입니다.**

공개된 Chrome Web Store(CWS) API(v2)에는 등록 정보를 읽거나 쓰는 기능이 없고, Chrome은 확장 프로그램이 CWS 페이지를 조작하는 것도 허용하지 않습니다.
그래서 Edictora는 전용 프로필의 Chrome을 Chrome DevTools Protocol(CDP)로 제어하고, 사람이 하는 것과 같은 입력으로 개발자 대시보드를 편집합니다.
단일 명령줄 도구(`edictora.exe`)로 동작합니다. Rust로 작성되었으며 .NET이나 Node.js 같은 런타임이 필요하지 않습니다.

> [!IMPORTANT]
> Edictora는 개발자 대시보드의 레이아웃에 의존합니다. **일본어**로 표시한 대시보드에서 검증했습니다.
> 영어 대시보드도 인식하지만, 영어 화면 문구는 아직 실제 화면에서 검증하지 않았습니다.
> Edictora가 필드나 버튼을 찾지 못하면(예: 대시보드가 개편된 뒤) 아무것도 변경하지 않고 중지합니다.

## 기능

- **pull**: 대시보드의 현재 값(설명, 공식 URL, 홈페이지 URL, 지원 URL)을 읽습니다. 읽기 전용입니다.
- **diff**: 원본 파일과의 차이를 표시합니다. 읽기 전용이며 기본 명령입니다.
- **push**: 차이가 있는 필드만 다시 쓰고 **초안으로 저장**한 뒤, 페이지를 다시 불러와 저장된 값을 원본 파일과 대조합니다.
- **이미지**: 설정의 항목에 `images`가 있으면 `pull`, `diff`, `push`가 스토어 이미지(스토어 아이콘, 스크린샷, 작은 프로모션 타일, 마키)도 다룹니다. `--only text|images|privacy`와 `--image-scope all|global|localized`로 범위를 좁힐 수 있습니다.
- **'개인정보 보호' 탭**: 설정의 항목에 `privacy`가 있으면 `pull`, `diff`, `push`가 대시보드의 '개인정보 보호' 탭(Privacy practices: 단일 목적 설명, 권한별 사유, 데이터 사용 공개)도 다룹니다. `--only privacy`로 범위를 좁힐 수 있습니다. 임시 저장까지만 하며 심사에는 제출하지 않습니다.
- **submit**: 항목 하나를 심사에 제출합니다. "심사 후 자동으로 게시" 설정은 변경하지 않습니다.
- **login**: 전용 프로필로 Chrome을 열어 Google에 직접 로그인할 수 있게 합니다.
- **update**: 새 버전이 있으면 다운로드하여 설치합니다. Setup으로 Edictora를 설치한 경우에만 동작하며, Windows에 로그인할 때도 자동으로 실행됩니다.

## 표시 언어

Edictora의 메시지, diff 출력, 도움말은 일본어, 영어, 중국어 간체, 중국어 번체, 한국어, 스페인어, 프랑스어, 독일어, 브라질 포르투갈어의 9개 언어로 제공됩니다.

- 기본적으로 Edictora는 다음 순서로 언어를 결정합니다. 환경 변수 `EDICTORA_LANGUAGE`, 그다음 `LC_ALL`, `LC_MESSAGES`, `LANG`(비어 있는 값, `C`, `POSIX`는 건너뜁니다), 그다음 Windows 표시 언어입니다. 지원하는 언어가 하나도 나오지 않으면 영어를 사용합니다.
- 언어를 직접 지정하려면 `EDICTORA_LANGUAGE`를 `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de`, `pt-BR` 중 하나로 설정합니다. 이전 이름인 `EDICTORA_LANG`도 읽지만 `EDICTORA_LANGUAGE`가 우선합니다.
- `edictora mcp`는 도구 설명과 응답을 AI 에이전트가 읽으므로, 표시 언어와 관계없이 항상 영어로 응답합니다.
- 이것은 개발자 대시보드의 언어(일본어 또는 영어), 그리고 등록 정보의 `locales`와는 별개입니다.
- 로그 파일은 작성자가 문제를 해결하기 위한 용도이므로 고정 메시지는 일본어로 유지됩니다. 로그에 포함되는 오류 메시지는 표시 언어를 따릅니다.

## 설치와 업데이트

`Edictora-win-Setup.exe`(Velopack 설치 프로그램)를 실행합니다. 관리자 권한 없이 `%LOCALAPPDATA%\Edictora`에 설치됩니다.

- 설치 후에는 새로 연 명령 프롬프트나 PowerShell 창 어디서든 `edictora`를 사용할 수 있습니다. Setup이 `%LOCALAPPDATA%\Edictora\current`를 사용자 `Path`에 추가하기 때문입니다.
- Windows에 로그인하면 `edictora-updater.exe`가 창 없이 실행됩니다. `https://edictora.kagayoi.com`에 새 버전이 있으면 다운로드하고, 크기와 해시를 확인한 뒤 설치합니다.
  - HKCU의 `Run`에 `Edictora Updater`로 등록되므로, 작업 관리자의 "시작 앱"에서 끌 수 있습니다.
  - 결과는 로그에 기록됩니다.
- 수동으로 업데이트하려면 `edictora update`를 실행합니다. 이미 최신이거나 업데이트가 시작된 경우에는 표준 오류로 알리고 종료 코드 0으로 끝나며, 실패하면 종료 코드 1로 끝납니다. `edictora`가 종료된 뒤에 Update.exe가 파일을 교체합니다.
- Portable 버전(`Edictora-win-Portable.zip`)은 로그인 시 자동으로 업데이트되지 않으며 `Path`에도 추가되지 않습니다. 업데이트하려면 압축을 푼 폴더에서 `current\edictora.exe update`를 실행합니다. 소스에서 빌드한 실행 파일은 스스로 업데이트하지 않습니다.
- `login`, `pull`, `diff`, `push`, `submit`을 실행하는 동안에는 업데이트가 적용되지 않습니다. Edictora는 다운로드만 마치고 적용은 나중으로 미룹니다.
  - 업데이트를 적용하는 중에 이 명령 중 하나를 실행하면, Chrome을 열지 않고 종료 코드 1로 끝납니다. 잠시 기다린 뒤 다시 실행하십시오.
  - 업데이트 프로세스가 종료된 뒤에도 Update.exe가 파일을 교체하는 데 몇 초가 더 걸리는데, 이 틈은 Edictora가 보호할 수 없습니다. 업데이트 직후에는 잠시 기다린 뒤 명령을 실행하십시오.
- 제거하면 `%LOCALAPPDATA%\Edictora`와 `Run`, `Path` 항목이 삭제됩니다. 데이터(`%LOCALAPPDATA%\Edictora.Data`)와 설정(`%APPDATA%\Edictora`)은 남으므로, 더 이상 필요하지 않다면 직접 삭제하십시오.

## 요구 사항

- Windows 10 또는 Windows 11(x86_64)
- Google Chrome. Edictora가 자동으로 찾으며, 설정 파일의 `chromePath`로 지정하거나 `--chrome`으로 전달할 수도 있습니다.
- CWS 개발자 계정
- 소스에서 빌드하는 경우 Rust 1.100.0(`rust-toolchain.toml`에 고정되어 있습니다). Bun은 페이지 내 스크립트를 변경할 때만 필요합니다.

## 사용법

### 1. 설정 파일 작성

기본 위치는 `%APPDATA%\Edictora\config.json`이며, `--config`로 변경할 수 있습니다.
항목마다 각 로캘을 원본 파일에 대응시킵니다. **생략한 필드는 읽거나, 비교하거나, 쓰지 않습니다.**

```jsonc
{
  // Optional. Defaults to %LOCALAPPDATA%\Edictora.Data\chrome-profile
  "profileDir": "%LOCALAPPDATA%\\Edictora.Data\\chrome-profile",
  "items": [
    {
      "itemId": "abcdefghijklmnopabcdefghijklmnop",   // 32-character CWS item ID
      "name": "Replace Translator",                     // Optional (for display)
      "baseDir": "D:\\Dev\\ReplaceTranslator",          // Optional (base for relative paths; defaults to the config file's folder)
      "locales": {
        "en": {
          "description": {
            "file": "webstore/store-listing.en.txt",
            "sectionStart": "^■ Description",           // Start after this heading line
            "sectionEnd": "^■ Category"                 // Stop before this heading line
          },
          "homepageUrl": "https://example.com/",
          "supportUrl": "https://example.com/support"
        }
      }
    }
  ]
}
```

- `description.file`만 지정하면 파일 전체가 설명으로 사용됩니다.
- 구간을 추출할 때 Edictora는 시작 제목 뒤부터 종료 제목 앞까지의 줄을 가져옵니다. 앞뒤의 빈 줄과 줄 끝의 공백을 제거하고, 줄바꿈은 `\n`으로 통일합니다. 제목을 찾지 못하면 오류로 중지합니다.
- 키가 없는 URL 필드는 무시됩니다. `""`는 해당 필드를 비워야 한다는 뜻입니다.
- 알 수 없는 키(오타 등)는 오류입니다. 주석과 후행 쉼표는 허용됩니다.

#### 카테고리(선택)

항목에 `category`를 추가하면 스토어 카테고리(대시보드의 '카테고리' 필드, 모든 언어 공통)를 읽고 비교하고 초안으로 저장할 수 있습니다. 소문자 키로 적습니다. 다른 필드와 마찬가지로 **적지 않으면 카테고리는 건드리지 않습니다**.

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- 키: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. 알 수 없는 키는 설정 오류입니다(종료 코드 2). '카테고리 미선택'은 원본 값이 될 수 없습니다.
- 카테고리는 모든 언어에 공통이므로 언어별이 아니라 항목당 한 번만 읽고 씁니다. `--only text`에 포함되며 `--locale`로는 좁혀지지 않습니다. `push --yes`는 차이가 있을 때만 바꾸고, 초안으로 저장한 뒤 다시 불러와 한 번 더 대조합니다.
- `locales` 없이 `category`만 가진 항목도 쓸 수 있습니다.
- 대시보드가 일본어로 표시될 때만 동작합니다(영어 화면의 문구는 확인되지 않았습니다).

#### 스토어 이미지(선택)

항목에 `images`를 추가하면 스토어 이미지도 관리할 수 있습니다. 다른 항목과 마찬가지로 **쓰지 않은 키는 건드리지 않습니다**. 경로는 `baseDir` 기준입니다.

```jsonc
"images": {
  "icon": "icons/icon-128.png",                             // 128x128 (a transparent PNG is allowed)
  "smallTile": "webstore/images/tile-small.png",            // 440x280
  "marquee": "webstore/images/tile-marquee.png",            // 1400x560
  "globalScreenshots": { "dir": "webstore/images/en" },     // or { "files": ["a.png", "b.png"] }
  "localizedScreenshots": {                                 // zh_CN = zh-CN
    "ja": { "dir": "webstore/images/ja" },
    "zh_CN": { "dir": "webstore/images/zh_CN" }
  }
}
```

- `dir`은 폴더 바로 아래의 `screenshot-*.png`를 이름 오름차순으로 사용합니다(1~5장). `files`는 적은 순서대로 사용합니다(1~5장).
- PNG만 지원하며, 알파 없는 24비트여야 하고 크기는 스크린샷 1280x800 또는 640x400, 작은 타일 440x280, 마키 1400x560입니다. Edictora는 Chrome을 열기 전에 이를 검사하고, 같은 세트 안에 같은 이미지가 없는지도 확인합니다(종료 코드 2). 다만 스토어 아이콘(`icon`)은 예외로, 128x128이어야 하며 투명한 PNG(8비트 RGB 또는 RGBA)도 쓸 수 있습니다. 비교는 알파를 포함해 하며, 완전히 투명한 픽셀의 색은 무시합니다. 모든 언어가 공유하므로 `--image-scope global`에 포함됩니다.
- 항목에 `locales` 없이 `images`만 둘 수도 있습니다(이미지 전용).
- **이미지는 파일 바이트가 아니라 디코딩한 픽셀로 비교합니다.** 대시보드는 업로드한 PNG를 다시 인코딩해 저장하므로, 저장된 파일은 바이트가 원본과 다르지만 픽셀은 같습니다. Edictora는 대시보드의 각 이미지(`=s0` 원본, `*.googleusercontent.com`에만 HTTPS로 접속, 쿠키 없음)를 내려받아 픽셀 해시를 순서대로 비교합니다. 내려받거나 디코딩할 수 없는 이미지는 "비교 불가"이며, 일치로 보지도 교체하지도 않습니다.
- `push --yes`는 차이가 있는 슬롯만 통째로 교체하고(전부 삭제한 뒤 순서대로 업로드), 임시 저장한 다음 다시 불러와 확인합니다. 처리 순서는 스토어 아이콘 → 작은 타일 → 마키 → 모든 언어용 스크린샷 → 언어별 현지화 스크린샷입니다.
- **이미지 삭제는 대시보드에 즉시 반영됩니다**("되돌릴 수 없습니다"라고 확인하며, 저장하지 않고 다시 불러와도 슬롯이 비어 있었습니다). push가 중간에 멈추면 임시 저장본에서 이미지가 빠진 채 남을 수 있으니, 다시 실행해 마무리하세요. 심사 대기 중인 항목은 건드리지 않습니다.
- 언어가 하나뿐인 항목에는 현지화 스크린샷이 없으므로 `--image-scope global`을 사용하세요. `localizedScreenshots`의 언어가 대시보드에 없으면 아무것도 바꾸기 전에 멈춥니다. `--image-scope global` 또는 `--locale`을 사용하세요.
- 이미지 쓰기는 대시보드가 일본어로 표시될 때만 동작합니다(영어 문구는 확인되지 않았습니다).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### '개인정보 보호' 탭(선택)

확장 프로그램에 권한을 추가하면, 대시보드의 **'개인정보 보호' 탭**(Privacy practices)에 권한 사유 등을 입력해야 심사에 제출할 수 있습니다. 항목에 `privacy`를 추가하면 해당 입력란을 원본 파일에서 읽고 비교하고 임시 저장할 수 있습니다. 다른 항목과 마찬가지로 **쓰지 않은 키는 건드리지 않습니다**.

```jsonc
"privacy": {
  // Text fields use the same "file + sectionStart / sectionEnd" as `description`
  "singlePurpose": { "file": "webstore/store-listing.txt", "sectionStart": "^■ Single purpose$", "sectionEnd": "^■ Data usage$" },
  "permissions": {                                  // manifest permission name -> justification; host permissions use the special key "host"
    "activeTab": { "file": "webstore/store-listing.txt", "sectionStart": "^  activeTab:$", "sectionEnd": "^  scripting:$" },
    "host": { "file": "webstore/store-listing.txt", "sectionStart": "^  Host permissions:$", "sectionEnd": "^■ Privacy policy URL$" }
  },
  "dataUsage": {
    "collected": ["personallyIdentifiable", "authentication"],   // the kinds to check; the rest are unchecked
    "certify": true                                            // checks the three declarations; only true is allowed
  },
  "policyUrl": "https://example.com/privacy"            // absolute http(s) URL; "" means the field should be empty
}
```

- 본문은 `description`과 같이 파일에서 구간을 잘라 냅니다(위 참조). 앞쪽 빈 줄을 없애고 모든 줄에 공통인 들여쓰기를 제거하므로, `  activeTab:` 아래에 들여써서 적은 사유도 그대로 쓸 수 있습니다. 어떤 본문도 비어 있으면 안 되며 1,000자 이내여야 합니다. Edictora는 Chrome을 열기 전에 이를 확인합니다(종료 코드 2).
- `collected`에는 `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity`, `websiteContent`를 쓸 수 있습니다(쓴 것만 체크하고 나머지는 해제합니다). `certify`는 `true`만 지정할 수 있습니다('판매하지 않음' 등 세 가지 선언에 체크한다는 의미). `false`나 생략은 설정 오류입니다.
- **권한 목록 자체도 대조합니다.** `permissions`를 쓴 경우, 설정에는 있는데 화면에 없는 권한(또는 그 반대)은 비교 결과에 표시되고 `push`는 아무것도 쓰지 않고 멈춥니다. 설정에 추가하거나 manifest를 고치세요.
- `policyUrl`은 개인정보처리방침 URL 필드입니다. http(s) 절대 URL(2,048자 이하) 또는 비워 두는 것이 원본임을 뜻하는 `""`. 적지 않으면 건드리지 않습니다. 이 탭의 다른 필드와 함께 초안으로 저장됩니다.
- `remoteCode`(`{ "uses": false }` 또는 `{ "uses": true, "justification": {...} }`)는 설정에 쓸 수 있지만, 대시보드의 해당 입력란을 실제 화면에서 확인하지 못했기 때문에 쓰면 `diff`가 오류를 보고하고 `push`가 멈춥니다. 지금은 쓰지 마세요.
- 대시보드가 일본어로 표시될 때만 사용할 수 있습니다(영어 문구는 확인되지 않았습니다). 임시 저장까지만 하며, Edictora는 절대 심사에 제출하지 않습니다.
- 항목에 `locales`나 `images` 없이 `privacy`만 둘 수도 있습니다('개인정보 보호' 탭 전용).

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. 로그인

```powershell
edictora login
```

전용 프로필로 Chrome이 열립니다. 그곳에서 Google에 로그인하고 개발자 대시보드를 여십시오.
- Edictora는 대시보드가 표시된 것을 확인하면 로그인 정보를 프로필에 보관하고 종료합니다.
- 기본적으로 최대 10분 동안 기다립니다. `--wait <minutes>`로 1~120분 범위에서 지정할 수 있습니다.
- Edictora는 비밀번호나 인증 코드를 다루지 않습니다. 직접 입력하십시오.

### 3. 비교와 업데이트

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| 종료 코드 | 의미 |
|---|---|
| 0 | 성공(차이 없음) |
| 1 | 실행 오류(대시보드 조작 실패, 검증 불일치 등) |
| 2 | 잘못된 인수 또는 설정(Chrome을 열지 않음) |
| 3 | 차이 있음(`diff`, 또는 `--yes` 없는 `push`) |
| 4 | 쓰기 중단(심사 대기 중이거나 상태를 확인할 수 없음) |
| 5 | 로그인 필요(자동 로그인 대기가 끝나지 않았거나 작업 도중 로그인이 만료됨. 다시 실행하면 필요할 때 로그인 창이 열립니다) |
| 130 | 중단됨(Ctrl+C) |

진행 상황은 표준 오류로, 결과는 표준 출력으로 나옵니다.

`pull`, `diff`, `push`, `submit`은 Chrome을 창 없이(헤드리스) 실행하므로, Chrome이 작업 중인 화면 앞으로 튀어나오는 일이 없습니다.
동작을 지켜보려면 `--show-browser`를 추가하십시오. `login`은 직접 조작해야 하므로 항상 창을 표시합니다. Google 로그인이 만료되었으면 `pull`, `diff`, `push`, `submit`이 스스로 창이 있는 Chrome을 열고 로그인이 끝날 때까지(최대 10분) 기다린 뒤 창을 닫고 이어서 진행합니다. 로그인이 끝나지 않았을 때(시간 초과, 창을 닫음, 같은 프로필에 창 없는 Chrome이 남아 있어 창을 띄울 수 없음)나 작업 도중 로그인이 만료되었을 때(이 경우 창은 열리지 않으므로 다시 실행하십시오) 종료 코드 5로 멈춥니다.

> [!WARNING]
> `--show-browser`는 사람이 화면을 지켜보는 용도입니다. AI 에이전트나 스크립트에서 Edictora를 실행할 때는 사용하지 마십시오. 사용자의 화면에 Chrome 창이 나타나는데, AI는 그 창을 볼 수도 없습니다.
> 실패 원인을 조사하려면 오류 메시지와 실패 시 저장되는 스크린샷을 사용하십시오.
> - 오류 메시지에는 가리고 있는 요소, 요소의 영역, 뷰포트 크기가 포함됩니다.
> - 스크린샷은 `%LOCALAPPDATA%\Edictora.Data\diagnostics`에 저장되며, 오류 메시지에 경로가 표시됩니다. `pull`이나 `diff`가 대시보드를 읽지 못했을 때도 스크린샷이 저장됩니다.

### 4. AI 에이전트에서 사용하기(MCP)

`edictora mcp`는 표준 입출력으로 동작하는 MCP 서버입니다. Claude Code, Codex 등의 클라이언트에 등록해 두면, AI가 등록 정보를 편집할 때 Edictora를 찾아서 사용할 수 있습니다.

**Setup은 Claude Code와 Codex에 Edictora를 자동으로 등록합니다.** 단, 공식 CLI(`claude`와 `codex`)가 `PATH`에 있어야 합니다. 제거하면 Edictora를 가리키는 등록만 삭제됩니다.
- 설치할 때 CLI가 없었거나 등록이 제때 끝나지 않았다면, 다음 로그인 때 Edictora가 다시 시도합니다. 나중에 CLI를 설치한 경우도 마찬가지입니다.
- 등록이 추가된 뒤에 직접 삭제하면 Edictora는 다시 추가하지 않습니다.

Portable 버전이나 이 두 가지 외의 클라이언트(Claude Desktop, 게이트웨이 등)는 아래 방법 중 하나로 한 번 등록하십시오.
설치된 exe는 절대 경로로 지정합니다.
- Setup 버전: `%LOCALAPPDATA%\Edictora\current\edictora.exe`. 자동 업데이트로 파일이 교체되어도 이 경로는 바뀌지 않습니다.
- Portable 버전: 압축을 푼 폴더의 `current\edictora.exe`.

`PATH`의 `edictora`도 사용할 수 있지만, 설치 전에 시작된 클라이언트는 새 `PATH`를 인식하지 못하므로 절대 경로가 더 안전합니다.

**Claude Code**(모든 프로젝트)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

또는 `%USERPROFILE%\.codex\config.toml`에 추가하십시오(`<username>`을 바꾸십시오).

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop 등 JSON 파일로 설정하는 클라이언트**(`<username>`을 바꾸십시오):

```json
{
  "mcpServers": {
    "edictora": {
      "command": "C:\\Users\\<username>\\AppData\\Local\\Edictora\\current\\edictora.exe",
      "args": ["mcp"]
    }
  }
}
```

MCP 게이트웨이(Toolport 등)를 사용한다면, 게이트웨이에도 같은 명령(stdio, 인수 `mcp`)을 등록하십시오.

| 도구 | 기능 |
|---|---|
| `edictora_list_items` | 설정 파일이 대상으로 하는 내용(항목, 로캘, 원본 파일)을 표시합니다. 브라우저를 열지 않습니다. |
| `edictora_diff` | 원본 파일과의 차이를 표시합니다(읽기 전용) |
| `edictora_pull` | 현재 값을 표시합니다(읽기 전용) |
| `edictora_push` | 차이가 있는 필드를 다시 쓰고 초안으로 저장합니다. `confirm: true`가 없으면 차이만 표시하고 중지합니다(드라이 런). |
| `edictora_submit` | 항목 하나를 심사에 제출합니다. `item`과 `confirm: true`가 필요합니다. |

- 이 도구들은 CLI와 같은 코드, 같은 안전장치, 같은 종료 코드를 사용합니다. 각 결과는 종료 코드와 그 의미로 시작합니다. 차이 있음(3)은 오류로 취급하지 않습니다.
- 클라이언트는 브라우저 표시(`--show-browser`), `login` 실행, 프로필 또는 chrome.exe 지정을 선택할 수 없습니다. Chrome은 아래의 로그인용 창을 제외하면 항상 창 없이 실행됩니다.
- 로그인이 만료되면 Edictora가 사용자의 화면에 로그인용 Chrome을 열고 로그인이 끝날 때까지(최대 10분) 기다린 뒤 도구 호출을 이어서 처리합니다. 끝나지 않으면 도구가 터미널에서 `edictora login`을 실행하라고 안내합니다.
- 호출은 한 번에 하나씩 순서대로 실행됩니다. 실행 중인 호출은 `notifications/cancelled`로 취소할 수 있습니다.
- 모든 항목에 대한 `diff`는 몇 분이 걸릴 수 있으므로, 클라이언트의 시간 제한을 넉넉하게 설정하십시오.

## 안전장치

- 기본적으로 Edictora는 차이만 표시하고 아무것도 변경하지 않습니다. 쓰기는 `push --yes`로만, 제출은 `submit --item <id> --yes`로만 수행합니다.
- 초안 저장과 심사 제출은 별개의 작업입니다. 제출은 항목 하나씩 수행하며, 일괄 제출은 없습니다.
- 저장하기 전에 Edictora는 다시 쓴 각 필드가 원본 파일과 정확히 일치하는지 확인합니다. 저장한 뒤에는 페이지를 다시 불러와 한 번 더 확인합니다. 일치하지 않으면 저장하지 않거나 오류로 중지합니다.
- 로캘마다 Edictora는 언어를 전환하고, 필드를 다시 쓰고, 검증하고, 저장한 뒤, 다시 불러와 한 번 더 검증합니다. 한 로캘이 실패하면 그 항목의 나머지는 건너뜁니다.
  - 로캘 간 롤백은 없습니다. 실패 전에 저장된 로캘은 초안으로 저장된 채 남습니다.
  - 항목을 심사에 제출하기 전에는 아무것도 공개되지 않습니다.
- 항목의 상태가 "심사 대기 중"처럼 안전하게 편집할 수 있다고 확인되지 않으면 Edictora는 쓰지 않습니다. 심사를 취소하는 일은 절대 없습니다.
- 필드와 버튼은 레이블, 역할, 텍스트로 찾으며 CSS 클래스 이름은 사용하지 않습니다. 검색 결과가 정확히 하나의 요소가 아니면 Edictora는 중지합니다.
- Chrome은 자동화 플래그 없이 시작되고 정상적으로 종료됩니다. **프로필은 절대 삭제하지 않습니다.**
- 실패 시 찍은 스크린샷은 `%LOCALAPPDATA%\Edictora.Data\diagnostics`에 저장되며, 저장소에는 저장되지 않습니다.
- 이미지 갱신은 차이가 있는 슬롯만 교체합니다. 순서에는 손대지 않으며, 심사 제출도 호출할 수 없습니다(이미지 처리에는 그 경로가 없습니다).
- '개인정보 보호' 탭 업데이트는 차이가 있는 입력란만 바꾸고, 심사에 제출하지 않으며(코드에 호출할 방법이 없음), 대시보드의 권한이 설정과 맞지 않으면 아무것도 쓰기 전에 멈춥니다.

## 파일 저장 위치

| 내용 | 위치 |
|---|---|
| 설정 파일(원본 파일 대응) | `%APPDATA%\Edictora\config.json` |
| Chrome 프로필(로그인 정보 포함) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| 로그(업데이트 결과 포함) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| 실패 시 스크린샷 | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| 앱 자체(Setup으로 설치한 경우) | `%LOCALAPPDATA%\Edictora`(제거 시 전체 삭제) |

제거할 때 로그인된 Chrome 프로필까지 함께 삭제되지 않도록, 데이터는 앱과 분리하여 `Edictora.Data`에 보관합니다. 이전 위치(`%LOCALAPPDATA%\Edictora\chrome-profile` 등)의 데이터는 자동으로 이전되지 않습니다.

Chrome 프로필에는 로그인 쿠키가 들어 있습니다. 공유하지 말고, 더 이상 필요하지 않으면 직접 삭제하십시오.
게시자 ID(대시보드 URL의 일부)는 로그에서 가려집니다.

### 설정 파일이 없는 것처럼 보일 때(Claude 데스크톱 앱)

Claude 데스크톱 앱은 패키지(MSIX) 앱입니다. 그 안에서 실행한 프로그램이 `%APPDATA%`나 `%LOCALAPPDATA%` 아래에 **새** 폴더를 만들면 Windows는 실제 위치가 아니라 앱 전용 저장소(`%LOCALAPPDATA%\Packages\<패키지>\LocalCache\...`)에 만들고, 일반 터미널에서는 보이지 않습니다. Edictora는 설정이나 Chrome 프로필을 그런 곳에서 읽고 있으면 표준 오류에 경고하고, 설정이 없으면 그곳에 있는 사본을 안내합니다. 해결하려면 메시지에 표시된 실제 위치(예: `%APPDATA%\Edictora\config.json`)로 그 파일(폴더)을 옮기세요. Edictora가 자동으로 읽거나 옮기지는 않습니다.

## 빌드

주요 프로그램은 Rust로 작성되어 있습니다(`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

대시보드 페이지 안에서 실행되는 스크립트는 `page-scripts/`(Bun + TypeScript)에 있습니다. 생성된 `page-scripts/dist/edictora.js`가 exe에 내장되므로, 이 스크립트를 변경할 때만 Bun이 필요합니다.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
```

**`sectionStart` / `sectionEnd`의 정규식**: Edictora는 Rust의 `regex` crate를 사용하므로, 전방 탐색, 후방 탐색(`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`), 역참조(`\1`)는 지원되지 않으며 설정을 불러올 때 오류가 됩니다. `^■ Description` 같은 일반적인 패턴은 그대로 동작합니다.

테스트용으로 다음을 재정의할 수 있습니다.
- 로컬 데이터 루트: `EDICTORA_LOCAL_ROOT`(기본값 `%LOCALAPPDATA%\Edictora.Data`)
- 로밍 루트: `EDICTORA_ROAMING_ROOT`(기본값 `%APPDATA%\Edictora`)
- 업데이트 소스: `EDICTORA_UPDATE_URL`(기본값 `https://edictora.kagayoi.com`. 평문 `http://`는 `127.0.0.1`에서만 허용됩니다)

Velopack 패키지에는 `edictora.exe`(주 exe)와 `edictora-updater.exe`가 모두 들어 있습니다.
- 업데이트에는 velopack crate를 사용하지 않습니다. Edictora가 Windows에 내장된 HTTP API인 WinHTTP로 피드를 가져오고 패키지를 다운로드하여 직접 검증합니다.
- 설치, 파일 교체, 제거는 Velopack의 Setup.exe와 Update.exe에 맡깁니다.

## 배포

릴리스는 Cloudflare R2의 `https://edictora.kagayoi.com`에서 제공됩니다(Velopack 채널 `win`). 릴리스 절차는 [docs/release.md](docs/release.md)를 참조하십시오.

## 라이선스

[LICENSE](LICENSE)를 참조하십시오. 개인정보 보호에 대해서는 [PRIVACY_POLICY.md](PRIVACY_POLICY.md)를 참조하십시오.
