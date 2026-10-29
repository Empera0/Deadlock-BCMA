import { ReactNode, useEffect, useState } from "react";
import { LANG, t } from "../i18n";

const SEEN_KEY = "guide-seen";

export type GuideTopic = "start" | "build" | "generator" | "draft" | "heroes" | "match" | "setup" | "settings";

interface Step {
  title: string;
  body: ReactNode;
}

type Topic = { key: GuideTopic; title: string; intro: string; steps: Step[] };

/** The guide in English; the Turkish one is TOPICS_TR, and one of the two is translated where shown by picking it. */
const TOPICS_EN: Topic[] = [
  {
    key: "start",
    title: "Getting started",
    intro: "Deadlock BCMA works out builds from the game's own numbers and from what top players buy, and studies your matches. Five screens, in the menu at the top (Ctrl K finds any screen or hero):",
    steps: [
      { title: "Build maker", body: "Put a build together item by item and see every number it gives: damage, health, resists, a fight against a target, the order to upgrade abilities in." },
      { title: "Build generator", body: "Let the app build for you: the popular build of top players, or the Master builder, which builds for one match against the enemy team you give it." },
      { title: "Draft", body: "During hero selection: which heroes do well into the enemy picks and with your team, and what the enemy will likely ban." },
      { title: "Hero tiers", body: "Every hero at the ranks you choose: how often it wins, is picked and banned, and what its players average." },
      { title: "Match analysis", body: "Open any match by its number, or your recent ones by your Steam account: how it went, why you died, what the build lacked." },
      {
        title: "First thing to set up",
        body: (
          <>
            Add <code>-condebug</code> to Deadlock's launch options in Steam (Properties, Launch Options). The app then reads the match you are in from the game's
            console log, finds your account on its own, and can build for your match as it starts. See <b>Setup</b> for the rest.
          </>
        ),
      },
      { title: "Open this guide again", body: "The ? at the top right opens it at the screen you are on, any time." },
    ],
  },
  {
    key: "build",
    title: "Build maker",
    intro: "Your build, with the game's numbers worked out for it.",
    steps: [
      { title: "Pick a hero and set the souls", body: "Click the hero at the top left to change it. The souls set the level and the ability points; leave them empty to follow what the items cost." },
      {
        title: "Buy items",
        body: "Click an item in the shop to buy it, and again to sell it. Search finds an item by name, stat or what it does. The bar at the top lists the build in order: there an item's effects that only work sometimes can be switched on, an imbued item can be put into an ability, and items can be moved or removed.",
      },
      {
        title: "Read the tabs on the right",
        body: (
          <>
            <b>Stats</b>: every stat with where it comes from, the next item worth buying, and the upgrade order for your abilities. <b>Combo</b>: line up abilities,
            shooting, melee and items and see what they deal to a target you set, step by step. <b>Misc</b>: how fast the build farms the jungle, and the Unstable Rift. <b>Compare</b>: set this build
            against a saved one or a friend's build code.
          </>
        ),
      },
      { title: "Buy, upgrade or sell", body: "At the bottom of the Stats tab, the advisor says which purchase, upgrade or swap adds the most to your fight, measured, not guessed." },
      { title: "Save and share", body: "Save builds by name, or copy the build code (BCMA1.…) and send it: pasting a code on the bar loads that build." },
    ],
  },
  {
    key: "generator",
    title: "Build generator",
    intro: "Builds made for you: the popular one, or one made for a match.",
    steps: [
      { title: "Popular", body: "What top players of the hero buy, phase by phase, with how often and how often they win with it. The core build is what nearly all of them buy." },
      {
        title: "Master builder",
        body: "Enter the enemy team (and your team, and who is in your lane, if you know them). The app builds phase by phase: what top players buy, what each item adds to your own fight, and what answers the enemy team.",
      },
      {
        title: "Focus and damage",
        body: "Focus is your role: carry, tempo, support or frontline (auto picks one from the teams). Damage is gun, spirit, hybrid or melee (auto follows what top players of the hero play). Pick spirit and the build is made for spirit damage even on a gun hero.",
      },
      {
        title: "Gold COUNTER items",
        body: "Items marked COUNTER are there for the enemy team: healing reduction against healers, an answer to their stuns, bullet resist against guns. They come at the end of their phase.",
      },
      {
        title: "Why not something else?",
        body: "Under each phase, the best items not bought and why: what they lost to, by popularity, by what they add to you, by their answer to the enemy team, or by price.",
      },
      {
        title: "Your live match",
        body: "With -condebug set, Load my live match fills both teams from the match you are playing, about a minute after it starts. Watch for my match does it on its own every match while the screen is open. The lobby list shows every player's rank, record on the hero and party.",
      },
      {
        title: "How items are measured",
        body: "Items are measured on a team fight: abilities that hit an area count for every enemy they hit (measured on match replays), stuns count for the time the enemy cannot fight back.",
      },
    ],
  },
  {
    key: "draft",
    title: "Draft",
    intro: "Which hero to pick while the draft is on.",
    steps: [
      { title: "Enter the picks", body: "Add the enemy picks and your team's picks as they come in; bans too. Load my live match fills them from the game when -condebug is set." },
      { title: "Pick the ranks", body: "Win rates come from the last 30 days of matches in the ranks you choose at the top, from everyone who played them. Your own matches are not needed." },
      { title: "Read the list", body: "Heroes are ranked by how they do into the enemy picks and in your lane. Each line says what it counts, with how many matches stand behind it." },
      { title: "Fit with the placed heroes", body: "The table shows, for each hero you placed, how every other hero has done beside it (an ally) or against it (an enemy). Green and red mark what the heroes' own strength does not explain." },
      { title: "Your own record", body: "Your Steam account ID adds your own record on each hero to the ranking. It is optional." },
      { title: "Then build", body: "Open the Master builder from the draft for the hero you pick: the teams carry over." },
    ],
  },
  {
    key: "heroes",
    title: "Hero tiers",
    intro: "Every hero at the ranks you choose, from the last 30 days of matches.",
    steps: [
      { title: "Pick the ranks", body: "The same range as the Draft screen: changing it on one changes it on the other." },
      { title: "Tiers", body: "Each hero is compared with the average one. S and A win clearly more, C and D clearly less. A hero played rarely is pulled toward 50% so a lucky run does not put it on top." },
      { title: "A hero's page", body: "Click a hero: its numbers, the heroes it is strong and weak against, its best teammates and its popular build. Open its build from there." },
      { title: "Items", body: "The Items tab ranks the shop at the same ranks, for every hero or one: win rate, how many buy it and when. Items are compared within their own cost, as expensive ones are bought by players already ahead." },
    ],
  },
  {
    key: "match",
    title: "Match analysis",
    intro: "What happened in a match and what to do differently.",
    steps: [
      { title: "Open a match", body: "Type a match number, or your Steam account ID for your recent matches. Matches stay on this computer once opened." },
      { title: "The match in short", body: "The story first: the lanes, the soul lead, where it turned, who carried. Then each player: build timing, team fights joined and missed, map play, objectives, titles." },
      {
        title: "Deaths",
        body: "Every death with who was around and where. Caught alone means outside a team fight with no teammate near; outnumbered in lane is a gank in your own lane, not your mistake.",
      },
      {
        title: "Why did you die?",
        body: "Each death is a duel: how fast the killer's full fight kills you, and how fast yours would have killed them, with the builds of the time. The items listed would have turned it or changed it most; ones that only keep you alive a second longer are left out.",
      },
      {
        title: "Against top players",
        body: "Each item you bought, against the minute top players of your hero buy it: early, on time, late or very late, and the items most of them buy that you never did. An item rushed ahead of its time (a counter) pushes back the ones after it; that delay is worked out from your soul income and not held against you.",
      },
      {
        title: "From the replay",
        body: "When Valve kept the match's replay, Study the replay shows every hit of the last ten seconds before each death (who, with what, how much health was left) and every stun, sleep, silence or hold put on you, and for each of your abilities how many enemies a cast caught and held against what a cast of it usually catches. Casts are judged by what they did, not by how often you used them.",
      },
      { title: "Why you die over many matches", body: "From the recent matches list: who kills you most, with what kind of damage, and the item that would have helped most often." },
      { title: "Open the build", body: "Open final build in the builder loads a player's build of that match into the Build maker." },
    ],
  },
  {
    key: "setup",
    title: "Setup",
    intro: "What the app needs from your computer and the game.",
    steps: [
      {
        title: "-condebug",
        body: (
          <>
            In Steam, open Deadlock's Properties and add <code>-condebug</code> to Launch Options, then start the game again. The game then writes a console log
            the app reads: the match you are in and the account you play on. Live match, Watch for my match and sharing your matches need it.
          </>
        ),
      },
      {
        title: "Settings",
        body: "Everything you set once is on the Settings screen, kept on this computer: the language, your Steam account ID (the short number or the long 7656… one; with -condebug the app offers the one the game is signed in to), the Draft screen's rank, the Master builder's role and damage, and what runs in the background. To look at someone else's matches, type their ID on Match analysis: your own stays as it is.",
      },
      {
        title: "Share my finished matches",
        body: "In Settings. When a match ends, the app asks deadlock-api for it, which keeps the match and, when Valve kept one, its replay, so it can be studied. Only the match number is sent; untick it to stop.",
      },
      { title: "Updates", body: "The app updates itself and says what is new. The round arrow at the top right (Update data) fetches the latest heroes and items after a patch." },
    ],
  },
];

/** The guide in Turkish, written plainly; translated where shown. */
const TOPICS_TR: Topic[] = [
  {
    key: "start",
    title: "Başlarken",
    intro: "Deadlock BCMA, oyunun kendi sayılarından ve en iyi oyuncuların aldıklarından build çıkarır ve maçlarını inceler. Üstteki menüde beş ekran var (Ctrl K ile her ekranı ya da heroyu bulabilirsin):",
    steps: [
      { title: "Build kur", body: "Eşya eşya bir build kur ve verdiği her sayıyı gör: hasar, sağlık, dirençler, bir hedefe karşı savaş, yeteneklerin hangi sırayla yükseltileceği." },
      { title: "Build önerici", body: "Build'i uygulama kursun: en iyi oyuncuların popüler build'i ya da senin verdiğin rakip takıma göre maç için build kuran Usta build." },
      { title: "Kahraman seçimi", body: "Kahraman seçerken: hangi kahraman rakiplere karşı ve takımınla iyi gider, rakip büyük ihtimalle neyi yasaklar." },
      { title: "Hero sıralaması", body: "Seçtiğin rütbelerde her hero: ne kadar kazanıyor, ne sıklıkla seçiliyor ve yasaklanıyor, oyuncuları ortalama ne yapıyor." },
      { title: "Maç analizi", body: "Herhangi bir maçı numarasıyla aç ya da Steam hesabınla son maçlarına bak: maç nasıl geçti, neden öldün, build'de ne eksikti." },
      {
        title: "İlk yapılacak ayar",
        body: (
          <>
            Steam'de Deadlock'un başlatma seçeneklerine <code>-condebug</code> ekle (Özellikler, Başlatma Seçenekleri). Uygulama böylece oynadığın maçı oyunun konsol
            kaydından okur, hesabını kendisi bulur ve maç başlarken senin maçına göre build kurabilir. Gerisi için <b>Kurulum</b> bölümüne bak.
          </>
        ),
      },
      { title: "Bu rehberi tekrar aç", body: "Sağ üstteki ? rehberi bulunduğun ekranda her zaman açar." },
    ],
  },
  {
    key: "build",
    title: "Build kur",
    intro: "Senin build'in ve oyunun sayıları, onun için hesaplanmış hali.",
    steps: [
      { title: "Kahraman seç, ruhu ayarla", body: "Değiştirmek için sol üstteki kahramana tıkla. Ruh, seviyeyi ve yetenek puanlarını belirler. Boş bırakırsan eşyaların fiyatına göre ayarlanır." },
      {
        title: "Eşya al",
        body: "Almak için mağazada bir eşyaya tıkla, satmak için tekrar tıkla. Arama, eşyayı adına, değerine ya da ne yaptığına göre bulur. Üstteki çubuk build'i sırayla gösterir: orada bir eşyanın bazen çalışan etkilerini açabilir, aktarılabilen bir eşyayı bir yeteneğe koyabilir, eşyaların yerini değiştirebilir ya da onları kaldırabilirsin.",
      },
      {
        title: "Sağdaki sekmeler",
        body: (
          <>
            <b>Değerler</b>: her değer ve nereden geldiği, alınmaya değer sonraki eşya ve yeteneklerinin yükseltme sırası. <b>Kombo</b>: yetenekleri, ateşi, yakın
            dövüşü ve eşyaları sıraya koy, ayarladığın hedefe adım adım ne yaptıklarını gör. <b>Diğer</b>: build'in ormanı ne kadar hızlı kestiği ve Unstable Rift.{" "}
            <b>Karşılaştır</b>: bu build'i kayıtlı bir build ya da bir arkadaşının build koduyla karşılaştır.
          </>
        ),
      },
      { title: "Al, yükselt ya da sat", body: "Değerler sekmesinin altında, öneri kutusu hangi alımın, yükseltmenin ya da değişimin savaşına en çok kattığını söyler. Tahmin değil, ölçüm." },
      { title: "Kaydet ve paylaş", body: "Build'leri adıyla kaydet ya da build kodunu (BCMA1.…) kopyalayıp gönder. Çubuğa bir kod yapıştırınca o build açılır." },
    ],
  },
  {
    key: "generator",
    title: "Build önerici",
    intro: "Senin için kurulan build'ler: popüler olan ya da maça göre kurulan.",
    steps: [
      { title: "Popüler", body: "O kahramanı oynayan en iyi oyuncuların aşama aşama aldıkları, ne sıklıkla aldıkları ve bununla ne sıklıkla kazandıkları. Ana build, neredeyse hepsinin aldığı eşyalar." },
      {
        title: "Usta build",
        body: "Rakip takımı gir (biliyorsan kendi takımını ve lane'indekileri de). Uygulama aşama aşama kurar: en iyi oyuncuların aldıkları, her eşyanın senin savaşına kattığı ve rakip takıma neye cevap verdiği.",
      },
      {
        title: "Rol ve hasar",
        body: "Rol: taşıyıcı, tempo, destek ya da ön saf (otomatik, takımlara göre birini seçer). Hasar: silah, büyü, karma ya da yakın dövüş (otomatik, en iyi oyuncuların oynadığına uyar). Büyü seçersen, silah kahramanı olsa bile build büyü hasarına göre kurulur.",
      },
      {
        title: "Altın KARŞI eşyalar",
        body: "KARŞI yazan eşyalar rakip takım için var: iyileşenlere iyileştirme azaltma, sersemletmelere cevap, silahlara mermi direnci. Aşamalarının sonunda gelirler.",
      },
      {
        title: "Neden başka bir şey değil?",
        body: "Her aşamanın altında alınmayan en iyi eşyalar ve nedeni: neye kaybettikleri; popülerlik, sana kattıkları, rakip takıma cevapları ya da fiyat yüzünden.",
      },
      {
        title: "Canlı maçın",
        body: "-condebug ayarlıysa Canlı maçımı yükle, oynadığın maçtan iki takımı doldurur, maç başladıktan yaklaşık bir dakika sonra. Maçımı takip et, ekran açıkken bunu her maçta kendisi yapar. Lobi listesi her oyuncunun rütbesini, kahramandaki sonucunu ve grubunu gösterir.",
      },
      {
        title: "Eşyalar nasıl ölçülür",
        body: "Eşyalar bir takım savaşında ölçülür: alana vuran yetenekler vurdukları her düşman için sayılır (maç tekrarlarından ölçüldü), sersemletmeler de düşmanın karşılık veremediği süre kadar.",
      },
    ],
  },
  {
    key: "draft",
    title: "Kahraman seçimi",
    intro: "Seçim sürerken hangi kahramanı alacağın.",
    steps: [
      { title: "Seçimleri gir", body: "Rakip seçimlerini ve takımının seçimlerini geldikçe ekle, yasakları da. -condebug ayarlıysa Canlı maçımı yükle bunları oyundan doldurur." },
      { title: "Rütbeleri seç", body: "Kazanma oranları, üstte seçtiğin rütbelerde son 30 günde oynanan herkesin maçlarından gelir. Kendi maçlarına gerek yok." },
      { title: "Listeyi oku", body: "Kahramanlar rakip seçimlere karşı ve lane'inde ne kadar iyi gittiklerine göre sıralanır. Her satır neyi saydığını ve arkasında kaç maç olduğunu söyler." },
      { title: "Seçilen herolarla uyum", body: "Tablo, eklediğin her hero için diğer her heronun onunla birlikte (takım arkadaşı) ya da ona karşı (rakip) nasıl gittiğini gösterir. Yeşil ve kırmızı, heroların kendi gücüyle açıklanmayan farkı gösterir." },
      { title: "Kendi sonucun", body: "Steam hesap numaran, her kahramandaki kendi sonucunu da sıralamaya ekler. İstersen." },
      { title: "Sonra build", body: "Seçtiğin kahraman için seçim ekranından Usta build'i aç: takımlar da gelir." },
    ],
  },
  {
    key: "heroes",
    title: "Hero sıralaması",
    intro: "Seçtiğin rütbelerde her hero, son 30 günün maçlarından.",
    steps: [
      { title: "Rütbeleri seç", body: "Kahraman seçimi ekranıyla aynı aralık: birinde değiştirince öbüründe de değişir." },
      { title: "Seviyeler", body: "Her hero ortalama bir heroyla karşılaştırılır. S ve A belirgin şekilde daha çok, C ve D belirgin şekilde daha az kazanır. Az oynanan bir hero %50'ye doğru çekilir, şanslı birkaç maçla tepeye çıkmasın diye." },
      { title: "Hero sayfası", body: "Bir heroya tıkla: değerleri, karşısında güçlü ve zayıf olduğu herolar, en iyi takım arkadaşları ve popüler build'i. Build'ini oradan açabilirsin." },
      { title: "Eşyalar", body: "Eşyalar sekmesi dükkânı aynı rütbelerde sıralar, bütün herolar ya da tek bir hero için: kazanma oranı, ne kadar alındığı ve ne zaman. Eşyalar kendi fiyatındakilerle karşılaştırılır, çünkü pahalıları zaten önde olanlar alır." },
    ],
  },
  {
    key: "match",
    title: "Maç analizi",
    intro: "Bir maçta ne oldu ve neyi farklı yapmalısın.",
    steps: [
      { title: "Maç aç", body: "Bir maç numarası yaz ya da son maçların için Steam hesap numaranı. Açılan maçlar bu bilgisayarda kalır." },
      { title: "Kısaca maç", body: "Önce hikâye: lane'ler, ruh farkı, maçın döndüğü an, kim taşıdı. Sonra her oyuncu: build zamanlaması, katılınan ve kaçırılan takım savaşları, harita oyunu, yapılar, unvanlar." },
      {
        title: "Ölümler",
        body: "Her ölüm, etrafta kimin olduğu ve nerede olduğu. Yalnız yakalandı: takım savaşı dışında, yakında takım arkadaşı yok. Lane'de sayıca az: kendi lane'inde yenen gank, senin hatan değil.",
      },
      {
        title: "Neden öldün?",
        body: "Her ölüm bir düello: öldürenin bütün gücü seni ne kadar hızlı öldürür, seninki onu ne kadar hızlı öldürürdü, o anki build'lerle. Listelenen eşyalar düelloyu çevirirdi ya da en çok değiştirirdi. Seni sadece bir saniye daha uzun yaşatanlar listelenmez.",
      },
      {
        title: "En iyi oyunculara karşı",
        body: "Aldığın her eşya, o kahramanı oynayan en iyi oyuncuların aldığı dakikaya göre: erken, zamanında, geç ya da çok geç. Ayrıca çoğunun aldığı ama senin hiç almadığın eşyalar. Zamanından önce alınan bir eşya (bir karşı eşya) arkasından gelenleri geciktirir. Bu gecikme ruh gelirinden hesaplanır ve sana yazılmaz.",
      },
      {
        title: "Tekrardan",
        body: "Valve maçın tekrarını sakladıysa Tekrarı incele her ölümden önceki son on saniyedeki her vuruşu gösterir (kim, neyle, ne kadar sağlık kaldı), sana yapılan her sersemletme, uyutma, susturma ya da tutmayı da. Her yeteneğin bir kullanımda kaç düşman yakaladığını ve tuttuğunu, o yeteneğin genelde yakaladığıyla karşılaştırır. Kullanımlar ne yaptıklarına göre değerlendirilir, ne sıklıkla kullandığına göre değil.",
      },
      { title: "Birçok maçta neden ölüyorsun", body: "Son maçlar listesinden: seni en çok kim öldürüyor, ne tür hasarla ve en sık hangi eşya işe yarardı." },
      { title: "Build'i aç", body: "Son build'i build kurucuda aç, o maçtaki bir oyuncunun build'ini Build kur ekranına yükler." },
    ],
  },
  {
    key: "setup",
    title: "Kurulum",
    intro: "Uygulamanın bilgisayarından ve oyundan istedikleri.",
    steps: [
      {
        title: "-condebug",
        body: (
          <>
            Steam'de Deadlock'un Özellikler'ini aç ve Başlatma Seçenekleri'ne <code>-condebug</code> ekle, sonra oyunu yeniden aç. Oyun böylece uygulamanın okuduğu bir
            konsol kaydı yazar: içinde olduğun maç ve oynadığın hesap. Canlı maç, Maçımı takip et ve maçlarını paylaşmak buna ihtiyaç duyar.
          </>
        ),
      },
      {
        title: "Ayarlar",
        body: "Bir kez ayarladığın her şey Ayarlar ekranında, bu bilgisayarda saklanır: dil, Steam hesap numaran (kısa numara ya da 7656… ile başlayan uzun numara; -condebug varsa uygulama oyunda açık olan hesabı önerir), Kahraman seçimi ekranının rütbesi, Usta build'in rolü ve hasarı, arka planda çalışanlar. Başka birinin maçlarına bakmak için onun numarasını Maç analizi ekranına yaz: seninki olduğu gibi kalır.",
      },
      {
        title: "Biten maçlarımı paylaş",
        body: "Ayarlar'da. Bir maç bitince uygulama bunu deadlock-api'ye bildirir. Servis maçı ve, Valve sakladıysa, tekrarını saklar, böylece incelenebilir. Sadece maç numarası gönderilir. Durdurmak için işareti kaldır.",
      },
      { title: "Güncellemeler", body: "Uygulama kendini günceller ve neyin yeni olduğunu söyler. Sağ üstteki yuvarlak ok (Veriyi güncelle), bir yamadan sonra en yeni kahramanları ve eşyaları indirir." },
    ],
  },
];

const TOPICS = LANG === "tr" ? TOPICS_TR : TOPICS_EN;

/** The guide: how to use each screen, opened on the first start and from the ? at the top right any time. */
export default function Guide({ open, topic, onClose, onTopic }: { open: boolean; topic: GuideTopic; onClose: () => void; onTopic: (t: GuideTopic) => void }) {
  if (!open) return null;
  // The Settings screen is explained under Setup.
  const shown = TOPICS.find((x) => x.key === (topic === "settings" ? "setup" : topic)) ?? TOPICS[0];
  return (
    <div className="overlay" onClick={onClose}>
      <div className="whats-new guide" role="dialog" aria-label={t("Guide")} onClick={(e) => e.stopPropagation()}>
        <div className="whats-new-head">
          <b>{t("Guide")}</b>
          <button className="ghost small" onClick={onClose}>
            {t("Close")}
          </button>
        </div>
        <div className="guide-body">
          <nav className="guide-topics">
            {TOPICS.map((x) => (
              <button key={x.key} className={"ghost small" + (x.key === shown.key ? " on" : "")} onClick={() => onTopic(x.key)}>
                {x.title}
              </button>
            ))}
          </nav>
          <section className="whats-new-body">
            <h3>{shown.title}</h3>
            <p className="muted">{shown.intro}</p>
            <ol className="guide-steps">
              {shown.steps.map((s) => (
                <li key={s.title}>
                  <b>{s.title}</b>
                  <p>{s.body}</p>
                </li>
              ))}
            </ol>
          </section>
        </div>
      </div>
    </div>
  );
}

/** Whether the guide should open on its own: the first time the app starts. */
export function useFirstStart(open: () => void) {
  useEffect(() => {
    try {
      if (localStorage.getItem(SEEN_KEY)) return;
      localStorage.setItem(SEEN_KEY, "1");
    } catch {
      return;
    }
    open();
  }, []);
}

/** A ? that opens the guide at a screen's topic. */
export function GuideButton({ onOpen }: { onOpen: () => void }) {
  return (
    <button className="ghost small guide-button" onClick={onOpen} title={t("How to use this screen")}>
      ?
    </button>
  );
}

export function useGuide(): { open: boolean; topic: GuideTopic; show: (t?: GuideTopic) => void; close: () => void; setTopic: (t: GuideTopic) => void } {
  const [open, setOpen] = useState(false);
  const [topic, setTopic] = useState<GuideTopic>("start");
  return {
    open,
    topic,
    show: (t) => {
      setTopic(t ?? "start");
      setOpen(true);
    },
    close: () => setOpen(false),
    setTopic,
  };
}
