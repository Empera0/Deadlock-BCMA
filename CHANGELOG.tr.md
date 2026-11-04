# Yenilikler

Uygulamanın Türkçe yenilik notları. Her sürümde neyin değiştiği, kısa ve sade.

## [0.9.0] – 2026-11-04

Maçtan sonra neyi düzelteceğin, bir yamanın ne yaptığı ve iki build yan yana.

### Eklenenler

1. **3 ders.** Bir maçın genel bakışının en üstünde: oyuncuya puandan en çok götüren üç şey, maçın kendi sayılarıyla (yalnızken ölümler, kaçırılan takım savaşları, lane, rakip takıma karşı eşyalar…), her biri gösterdiği bölüme bir bağlantıyla.
2. **Yama etkisi.** Hero sıralamasında yeni bir sekme: her hero ve eşyanın bir yamadan önceki günlerdeki kazanma ve seçilme oranı, sonraki günlerle yan yana, seçili ranklarda. Şanstan büyük değişiklikler belirgin olarak işaretli. Yama günleri oyunun yama notlarından gelir; biten günler bu bilgisayarda tutulur.
3. **Build karşılaştırma.** Build kur'daki Karşılaştır: şu anki build kayıtlı biriyle, ya da iki kayıtlı build. Her alımdan sonra savaş hasarı ve etkili can, harcanan ruha göre; hangisinin nerede öne geçtiği ve tamamlanmış build'lerin en çok ayrışan değerleri.

## [0.8.2] – 2026-11-03

### Eklenenler

1. **Eksik maçların gönderiliyor.** Biten maçlarımı paylaş açıksa uygulama açılınca ve her maçtan sonra son 30 maçına bakar, deadlock-api'de olmayanları gönderir: servis onları Steam'den alır, böylece herkes açabilir; maçlar bu bilgisayarda da tutulur. Ayarlar'da hemen yapmak için bir düğme var.

## [0.8.1] – 2026-11-03

Seçilecek daha çok görünüm ve daha sağlam bir Canlı ekranı.

### Eklenenler

1. **Daha çok tema.** Kızıl, Kor, Hidden King, Archmother, Mor ve Nord (koyu), Ayaz ve Kum (açık).
2. **Özel tema.** Arka plan, kart, yazı ve vurgu renklerini, istersen silah, dayanıklılık ve büyü renklerini de seç; koyu ya da açık. Gerisi bu renklerden hesaplanır. Herhangi bir temadan başlayabilirsin.
3. **Görünüm ayarları.** Bütün pencerenin boyutu (%80–140), keskin, normal ya da yuvarlak köşeler, yazı tipi ve animasyonları açıp kapatma.

### Düzeltilenler

1. **Canlı ekranı.** Takımlar önce Valve'ın canlı maç listesinden (oyundaki İzle sekmesi) okunur; maç oradaysa hemen gelir.
2. Uzun bir maçta oynanan maç, uygulamanın okuduğu konsol kaydının dışına düşüp bitmiş görünebiliyordu; kayıt artık kaldığı yerden okunuyor.
3. Bütün oyuncular gelmeden okunan takımlar öyle kalıyordu; artık gösteriliyor ve herkes gelene kadar tekrar soruluyor.
4. Canlı ekranındaki oyuncular, hero istatistikleri okunamasa da listeleniyor.

## [0.8.0] – 2026-11-03

Maç analizi maçın nasıl aktığını ve nasıl oynadığını, en iyi oyuncular ve kendi ortalamanla yan yana gösteriyor.

### Eklenenler

1. **Tekrar.** Yeni Tekrar sekmesi maçı mini haritada oynatır: herolar canlarıyla, ölümler ve düşen yapılar; çubuğun altında ölümler, yapılar ve takım savaşları işaretli. Birine tıklayınca 15 saniye öncesinden izlersin; bir heroya tıklayınca onu takip edersin.
2. **Alışkanlıklar.** Yeni Alışkanlıklar sekmesi her oyuncu için: zamanın nereye gittiği (savaş, askerler, orman, yolda, üste, beklemede, ölü) ve haritada nerede olduğu; lane, orta oyun, geç oyun ve bütün maç için, herodaki en iyi oyuncularla yan yana (birkaç yakın tarihli yüksek rank maçı, haftada bir okunur).
3. **Lane dakika dakika.** Lane rakiplerine karşı ilk on dakika: her dakika iki tarafın kaybettiği can ve takasta geçen süre, oyunun kayıtlarında ruh, son vuruş ve hasar, ve lane'in ne zaman döndüğü.
4. **Bu maç ve ortalaman.** Maçın dakika başı değerleri, oyuncunun o herodaki kendi ortalamasıyla yan yana.
5. **Ranklar.** Maç analizinde her oyuncunun rankı: ranked maçlarda maçın kaydettiği rank, diğerlerinde şu anki rankı, öyle işaretli.

## [0.7.0] – 2026-11-02

Maçın lobiden sonuca kendiliğinden takip ediliyor ve nasıl oynadığına daha yakından bakılıyor.

### Eklenenler

1. **Canlı ekranı.** Bir maç başlayınca uygulama Canlı ekranını kendiliğinden açar: iki takım oyuncu oyuncu (hero, rütbe, o herodaki sonuçlar, en çok oynadığı herolar, seriler, birlikte girenler) ve lane'in için bir not. Maç başında maçın yayınından bir kez okunur; maçı takip etmez. Build'imi planla, Usta build'i iki takımla açar.
2. **Lane notu.** Lane'indeki her rakip için: herona karşı lane'de ve maç genelinde nasıl gittiği, geç oyunda mı güçlendiği yoksa erken mi en güçlü olduğu, ve ona karşı alınıp her zamankinden çok kazandıran eşyalar.
3. **Maçtan sonra.** Bir maç bitince sonucu, heronu, Ö/Ö/A'yı, ruhu ve sıralamanı gösteren bir kart çıkar; tam analize giden bir düğmeyle.
4. **Benim herolarım.** Hero sıralamasında oynadığın her hero, rütbendeki ortalama oyuncuyla yan yana: kazanma, öldürme, ölüm, asist, ruh, kahraman hasarı, son vuruş ve isabet.
5. **Zaman içinde.** Son maçlar raporu kazanma oranını, puanını, ölümlerini ve ilk büyük eşyanın dakikasını maç maç grafikle gösterir.
6. **Build resmi.** Paylaş'ın yanındaki Resim, build'i bir resim olarak kopyalar (hero, sırayla eşyalar, temel değerler, kod); bir sohbete yapıştırırsın.
7. **Yama uyarısı.** Bir yamadan sonra oynadığın herolarda ve kayıtlı build'lerindeki eşyalarda neyin değiştiğini bir kart listeler.
8. **Yedekleme.** Ayarlar'dan ayarlarını, build'lerini ve tutulan maçları tek dosyaya kaydedebilir, başka bir bilgisayarda geri yükleyebilirsin.
9. **Önbellekteki maçların paylaşımı.** Biten maçlarımı paylaş açıksa oyunda açtığın maçlar deadlock-api'ye bildirilir; böylece herkes okuyabilir.

## [0.6.1] – 2026-11-02

### Düzeltilenler

1. Deadlock birden fazla Steam kütüphanesinde kuruluysa canlı maç eski bir konsol kaydını okuyordu; artık en son yazılan kayıt okunuyor.

## [0.6.0] – 2026-11-02

Maçlar artık kendi bilgisayarından açılıyor, istatistik servisi onları tanımasa bile.

### Eklenenler

1. **Steam önbelleğinden maçlar.** Oyunda açtığın her maç (skor tablosu ya da tekrarı) bütün ayrıntılarını Steam'in bilgisayarındaki önbelleğine bırakır. Maç analizi artık önce oradan okuyor, sonra Valve'ın tekrar sunucularından, istatistik servisine en son soruyor; böylece servisin görmediği bir maç da açılıyor. "Bu bilgisayardaki maçlar" ayrıntıları zaten diskinde olanları işaretliyor.

## [0.5.3] – 2026-10-31

### Değişenler

1. **Canlı maç.** Maç oyunun konsol kaydından (-condebug) bulunuyor, takımları da sadece maçın yayınından okunuyor; oyunun en iyi maçlar izleme listesine bakılmıyor. Valve bir maçı yayınlamazsa uygulama bunu açıkça söylüyor: maç bulundu, takımları okunamıyor, elle girilebilir.

## [0.5.2] – 2026-10-30

### Değişenler

1. **6821 yamasının oyun verisi.** Uygulamanın içindeki herolar, yetenekler ve eşyalar ile oyunun Türkçe metinleri son yamaya uyuyor: 90 değişiklik. Aralarında Mo & Krill, Rat King ve Solomon'a nerf, güçlenen can çalma ve iyileşme azaltan eşyalar, pususu artık silah hasarı veren Shadow Weave var. Uygulama veriyi açılışta da kendisi indiriyor; Yama değişiklikleri hepsini listeliyor.

## [0.5.1] – 2026-10-29

### Düzeltilenler

1. **Rütbe isimleri.** 3 ile 7 arasındaki rütbelerin isimleri yanlıştı: doğrusu Acolyte, Sentinel, Mystic, Ritualist ve Emissary. Arkadaki numaralar doğruydu, yani istatistikler seçilen rütbelerden geliyordu; sadece isimler yanlış yazıyordu. Rütbe aralığında, build'lerin rütbe adlarında ve lobi taramasında düzeltildi.
2. Açılır listeler okunuyor: rütbe seçicileri Windows'ta beyaz bir liste üstünde açık renk yazıyla açılıyordu.

## [0.5.0] – 2026-10-29

Her heronun kendi sayfası var, dükkânın eşyalarının da bir sıralaması.

### Eklenenler

1. **Hero sayfaları.** Hero sıralamasında bir heroya tıkla: seçtiğin rütbelerdeki değerleri, karşısında en güçlü ve en zayıf olduğu herolar, en iyi takım arkadaşları (her biri heroların kendi gücünden beklenenin üstünde ya da altında) ve popüler build'i; build'i açan bir düğmeyle.
2. **Eşya sıralaması.** Yeni Eşyalar sekmesi dükkânı seçtiğin rütbelerde sıralar, bütün herolar ya da tek bir hero için: kazanma oranı, oyuncuların ne kadarının aldığı ve kaçıncı dakikada. Eşyalar kendi fiyatındakilerle karşılaştırılır, çünkü pahalıları zaten önde olanlar alır.

## [0.4.2] – 2026-10-29

### Düzeltilenler

1. Build kur'un altındaki satın alma sırası yeniden her eşyanın dükkândaki resmini gösteriyor, kategorisinin renginde çerçeveli. Önceden düz ikonlarla resimler karışık görünüyordu.

## [0.4.1] – 2026-10-29

### Düzeltilenler

1. Dükkândaki eşya açıklamaları yeniden okunuyor: değerler koyu zemin üstünde koyu yazılıyordu. Oyunun adsız iç değerleri artık gösterilmiyor ve kutu pencereden taşmıyor.
2. Build kur'un altındaki satın alma sırası artık kesilmiyor.

## [0.4.0] – 2026-10-29

Baştan aşağı yeni bir görünüm, oyundaki gibi çizilmiş bir dükkân ve üç yeni araç: hero sıralaması, yama değişiklikleri ve kendi rütbende build'ler.

### Eklenenler

1. **Hero sıralaması.** Yeni bir ekran: seçtiğin rütbelerde her hero. Kazanma, seçilme ve yasaklanma oranı, oyuncularının Ö/Ö/A ve ruhu, ortalama heroya göre S'den D'ye bir seviye. Az oynanan bir hero şanslı birkaç maçla tepeye çıkmaz. Bir heroya tıklayınca oyuncuların onunla ne build yaptığını görürsün.
2. **Yama değişiklikleri.** Bir yamadan sonra uygulama oyunun sayılarını öncesi ve sonrasıyla karşılaştırır: hero değerleri, her yetenek değeri ve geliştirmesi, eşya fiyatları ve değerleri, yeni gelenler ve kaldırılanlar. Sağ üstteki düğme bunları listeler; kayıtlı build'lerindekiler en üstte.
3. **Kendi rütbende build'ler.** Popüler ve Usta build'ler en iyi oyuncular yerine seçtiğin rütbe aralığından okunabilir. Build önericide ya da Ayarlar'da seçilir.
4. **Temalar.** Ayarlar'da altı görünüm (Obsidyen, Grafit, Gece Yarısı, Yosun, Erik ve açık renkli Kâğıt), sıkı ya da rahat yerleşimle.
5. **Arama.** Ctrl K ile her ekranı, heroyu ya da işlemi bulursun.

### Değişenler

1. **Yeni yerleşim.** Sol menü kalktı: ekranlar üstte sekmeler halinde, her ekranın araçları altında tek satırda, içerik yuvarlak kartlarda.
2. **Oyundaki gibi dükkân.** Build kur'daki dükkân oyundaki üç dükkân gibi çizildi (Fairfax, MPS ve Curiosity Catalog): çerçeveli kademe panelleri, fiyat etiketleri, kâğıt kartlar. Satın alma sıran altta, oyundaki eşya slotları gibi duruyor.
3. **Lobi gibi kahraman seçimi.** İki takım karşılıklı, kazanma şansı ortada; altında hero seçici, en altta öneriler ve uyum tablosu yan yana.
4. **Kartlara ayrılmış maç analizi,** kimin kazandığını gösteren bir başlıkla.

## [0.3.2] – 2026-10-28

Kahraman seçimi ekranı artık istediğin rütbe aralığına bakıyor ve heroların birbiriyle uyumunu gösteriyor.

### Eklenenler

1. **Seçilen herolarla uyum.** Kahraman seçimi ekranında yeni bir tablo: eklediğin her hero için diğer her heronun onunla birlikteyken (takım arkadaşı) ya da ona karşı (rakip) nasıl gittiği. Heroların kendi gücüyle açıklanmayan farklar yeşil ve kırmızı. Tablodaki bir heroya tıklayınca eklenir. Önerilen seçimleri değiştirmez, ayrıca gösterilir.
2. **Rütbe aralığı.** Seçim ekranı, seçtiğin rütbe aralığındaki (şundan şuna) maçlara bakar. Kahraman seçimi ekranında ya da Ayarlar'da varsayılan olarak seçilir. Kendi maçlarına hiç ihtiyacı yoktu; hesabın sadece her herodaki kendi sonucunu ekler.

### Düzeltilenler

1. Her hero seçime eklenebiliyor. Seçilen rütbelerde az maçı olan herolar gizleniyordu; artık sadece önerilen seçimlere girmiyorlar.

## [0.3.1] – 2026-10-28

Uygulama artık Türkçe konuşuyor.

### Eklenenler

1. **Türkçe.** Ayarlar'da dil seçimi var: English ya da Türkçe. Türkçe seçince bütün ekranlar, rehber, hata mesajları ve uygulamanın yazdığı cümleler (maçın hikâyesi, neden öldüğün, build notları, kombo kaydı) sade bir Türkçeyle yazılır. Sayılar ve tarihler de Türkçe yazılır. Eşya ve yetenek açıklamaları oyunun kendi Türkçesinden gelir.
2. **Adlar ayrı ayarlanır.** Kahraman, eşya ve yetenek adlarının kendi ayarı var: İngilizce oyundaki gibi ya da Türkçe oyundaki gibi (Haze → Duman, Metal Skin → Metal Deri). Uygulama Türkçe, adlar İngilizce olabilir; ya da tersi. Build kodları iki durumda da çalışır.

### Değişenler

1. Otomatik kontroller artık Türkçesi eksik bir metin olunca da hata veriyor. Aylık veri yenilemesi oyunun Türkçe metinlerini ve adlarını da yeniliyor.
