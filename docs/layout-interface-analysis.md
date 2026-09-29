# Give workers the floor, experts the ceiling

*Research for D173's interface, 2026-09-29: how Apple makes complex software simple for beginners and deep for experts, and what that means for Spork's layout. The recommendation is at the end.*

Apple makes complex software easy for beginners and still rewarding for experts by keeping a single surface and grading its depth. It does not ship an "easy mode". A small, labelled core covers the work most people do most of the time. Advanced controls are disclosed in place, beside the thing they affect. Every faster route (shortcut, context menu, command search) duplicates a command that already has a visible home. Constraints make common mistakes impossible instead of reporting them, and unlimited undo makes exploring safe. Apple's own history is the sharpest evidence. Logic Pro carried a global Simplified/Complete switch for about twelve and a half years before Logic Pro 12 removed it in January 2026. The feature-cutting rewrites of Final Cut Pro X (2011) and iWork (2013) both had to restore features within months. Meanwhile selection-driven inspectors, live alignment guides and Final Cut's magnetic constraints survived every redesign. Independent research points the same way. Blocking error states speeds learning and carries over to the full system. A plain-language redesign more than halved task time for low- and high-literacy users alike. People prefer layers they choose over interfaces that rearrange themselves. And in a lab study, pickers were faster and far more accurate with a picture of the shelf face than with a text list.

For Spork we recommend **one place model, three depths, no modes**. Floor staff meet the layout as place pages reached by a scan: a breadcrumb, the rack face with the target cell lit, and a small plan. They correct it by scanning, never by dragging geometry on a handheld. Office users meet the same model as a 2D plan with a live 3D pane beside it. They get a selection-driven inspector with one "More options" disclosure, magnetic containment, a naming pattern written as a sentence with a live preview, and keyboard and search shortcuts. Drawing rather than measuring is also the right choice on capture grounds. Apple's RoomPlan scanner is designed for rooms of at most 15 m × 15 m × 3.6 m, and its sensor reaches about 5 m. The main uncertainty is that no study has tested layout editors with warehouse floor staff. Most of this recommendation therefore transfers by analogy, and it should be tested on the floor before it is polished.

## Apple has taught one surface with graded depth since 1992

Apple's written guidance has given the same answer to the novice-versus-expert question for more than thirty years. The 1992 *Macintosh Human Interface Guidelines* describe progressive disclosure as a way to "present the most common choices to users while initially hiding more complex choices". The aim is an interface that "is easy for novice users to learn and includes the features and power that advanced users desire". The prescribed control is a labelled **More Choices** button that becomes **Fewer Choices** ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). The 2005 Mac OS X edition adds a rule that runs against intuition: "The more complex your application's task, the more important it is to keep the user interface simple and focused" ([2005 HIG](http://www.multimedialab.be/doc/tech/doc_osx_hi_guidelines.pdf)).

Mike Stern's WWDC17 session gives the modern version. "It is okay to make the most useful 20 percent of functions easier to find by hiding the other 80 percent", so that "more experienced users can quickly reveal the options and actions that they require" ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)). Today's HIG puts the most-used controls "at the top of the disclosure hierarchy so they're always visible, with more advanced functionality hidden by default". The hidden part goes behind a label such as "Advanced Options", with **no more than one disclosure button in a view** ([HIG: Disclosure controls](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)).

Two guardrails stop this sliding into "hide everything". The first is about whom to design for. The 1992 guidelines' "80 percent solution" warns: "If you try to design for the 20 percent of your target audience who are power users, your design will not be usable by the majority of your users." It adds that those power users "probably think a lot like you" ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). That is a claim about *people*, not the later split of *functions* in Stern's talk, and the two should not be merged.

The second guardrail is availability. The 2005 edition says "a corollary of simplicity is availability". It tells designers to avoid hiding key features "too deeply in submenus or making them accessible only from a contextual menu" ([2005 HIG](http://www.multimedialab.be/doc/tech/doc_osx_hi_guidelines.pdf)). This echoes the 1992 rule that hidden features must tell people "where they can find more choices". On **8 June 2026** Apple reintroduced a design principles page that puts both guardrails in two sentences: "Simplicity isn't minimalism. Aim for a focused, useful experience that keeps the important things close by and lets the others fall away" ([HIG: Design principles](https://developer.apple.com/design/human-interface-guidelines/design-principles)).

Apple's designers also say where the hidden complexity goes: into the software. Larry Tesler's "law of conservation of complexity" says every application has an irreducible amount of complexity, and "the only question is: who has to deal with it" ([Wikipedia, secondary](https://en.wikipedia.org/wiki/Law_of_conservation_of_complexity)). Ken Kocienda describes the tuned numbers inside the original iPhone software as "sensible defaults, or pleasing effects, or a way to give people what they meant rather than what they did" ([Commoncog, reproducing *Creative Selection*](https://commoncog.com/creative-selection/)). Jony Ive introduced iOS 7 by saying true simplicity "is about bringing order to complexity" ([TechCrunch](https://techcrunch.com/2013/06/11/jony-ives-debutes-ios-7-bringing-order-to-complexity/)).

Direct manipulation is Apple's preferred way to spend that effort. The 1992 guidelines want an object to stay visible while it is acted on, with the result "immediately visible". Stern calls a text label beside a control "a telltale sign" of unclear mapping, because "reading takes time", and concludes that "the best mapping is the most direct mapping" ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)).

The last ingredient is safety. The 1992 principle of forgiveness asks for "safety nets for people so that they feel comfortable learning". It also warns that "frequent alert boxes are a good indication that something is wrong with the program design" ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). The 2026 Agency principle says "when people know they can reverse an action or return to a previous state, they feel free to explore" ([HIG: Design principles](https://developer.apple.com/design/human-interface-guidelines/design-principles)). The undo guidance asks for:

- no "unnecessary limits" on how far back people can undo;
- labels that name what will be undone ("Undo Typing");
- scrolling to show what an undo restored.

([HIG: Undo and redo](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo)). Modes are tolerated only in a narrow form. The 1992 guidelines accept short "spring-loaded" modes that last only while the user holds a key or button, and insist that any mode has "a clear visual indicator… near the object most affected" ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)).

Read together, the guidance describes a stack in which every layer serves both audiences. The table is our synthesis (inference); each rule in the last column is quoted from Apple.

| Layer | What the novice gets | What the expert gets | Apple's rule |
|---|---|---|---|
| Visible, labelled core | A few controls whose meaning is clear | The commonest actions one click away | Toolbar groups: "aim for a maximum of three"; "Don't make people guess" ([HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)) |
| One complete home for every command | Can browse to find out what exists | Sees each command's shortcut beside it | "Make every toolbar item available as a command in the menu bar" ([HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)) |
| Disclosure in place | Is not overwhelmed | Advanced options sit next to what they change | One labelled disclosure per view ([HIG: Disclosure controls](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)) |
| Inspector that follows the selection | Only the controls that apply | Every attribute of the selection | Inspectors "update dynamically based on the current selection" ([2005 HIG](http://www.multimedialab.be/doc/tech/doc_osx_hi_guidelines.pdf)) |
| Shortcuts that duplicate visible commands | Never required | Speed | A context menu "isn't for providing advanced or rarely used items" ([HIG: Context menus](https://developer.apple.com/design/human-interface-guidelines/context-menus)); a hidden Option-key item is never "the only way" ([HIG: The menu bar](https://developer.apple.com/design/human-interface-guidelines/the-menu-bar)) |
| Personalisation | Defaults that work untouched | A custom toolbar for long sessions | Customisation is "especially useful in apps… people tend to use for long periods of time" ([HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)); defaults should suit "the largest number of people" ([HIG: Settings](https://developer.apple.com/design/human-interface-guidelines/settings)) |
| Forgiveness | Safe to try things | Free to experiment quickly | Unlimited, named undo that shows its result ([HIG: Undo and redo](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo)) |

## Apple's reversals show that mode switches and feature cuts fail, while constraints last

Apple has tried the obvious alternative, a global beginner/expert switch, and has now removed it. Logic Pro X launched in 2013 with **Show Advanced Tools** plus six separately ticked "Additional Options". Apple's help marked every feature that needed them with an icon ([Apple Support PH24704](https://support.apple.com/kb/PH24704?locale=en_US&viewlocale=en_US)). By the 10.7 guide this had become a single **Enable Complete Features** checkbox. A visible **Simplified** button in the control bar showed the current state and opened the setting directly ([Apple Support, Logic Pro 10.7](https://support.apple.com/guide/logicpro/use-the-complete-set-of-logic-pro-features-lgcp5cbf192f/10.7/mac/11.0)).

The switch never earned its keep with experts. Sound On Sound told readers that the first thing to do was turn every option on (search excerpt) ([Sound On Sound](https://www.soundonsound.com/reviews/apple-logic-pro-x)). Berklee Online's courses "assume that the Complete Features checkbox is enabled" ([Berklee Online](https://online.berklee.edu/help/en_US/logic/1617594-how-to-turn-on-complete-features-in-logic-pro)). Users were still confused by the start-up prompt in December 2025 ([Apple Community](https://discussions.apple.com/thread/256217024)). Logic Pro 12, released in January 2026, ended it: "the complete set of features are now always available. The Advanced pane is no longer available in Logic Pro Settings" ([Apple Support, What's new in Logic Pro](https://support.apple.com/guide/logicpro/whats-new-in-logic-pro-lgcp4a62a494/mac)).

Apple gave no reason, so the cause is our inference. The costs visible in the record match what Shneiderman predicted for switching between layers (next section):

- a badge on every feature the switch hid;
- third-party instructions that begin "turn on the checkbox";
- people stuck in the wrong mode.

Two Logic behaviours outlived the switch and are worth copying:

1. **The project decided what appeared.** A project turned on "any additional options used by the project" automatically, and content built with hidden tools "will still play" when those tools were off ([Apple Support PH24704](https://support.apple.com/kb/PH24704?locale=en_US&viewlocale=en_US)).
2. **The beginner's app and the expert's app share one format.** GarageBand projects open in Logic with "no need to import or convert any files" ([Apple Support, GarageBand projects in Logic Pro](https://support.apple.com/guide/logicpro/garageband-projects-lgcpa8854ca7/mac)).

The other failure is simplifying by cutting features. Final Cut Pro X (June 2011) replaced tracks with a **magnetic timeline**, which works like this ([Apple Support, Intro to the Magnetic Timeline](https://support.apple.com/guide/final-cut-pro/intro-to-the-magnetic-timeline-verb8fcfc133/mac)):

- Clips "automatically move out of the way or snap together to avoid unwanted gaps and collisions."
- Connected clips move with the clip they are attached to, so dialogue stays "married to picture".
- A separate **Position tool** turns the magnetism off for one move and leaves a visible **gap clip** where the moved clip used to be.

Apple's stated aim was "removing the technical overhead of track management" for "both beginner creators and professional editors" (same source). The backlash was fierce, but it was driven by missing features (no import of old projects, no XML or EDL export at launch) and by Apple discontinuing the old app at once ([Daring Fireball](https://daringfireball.net/2011/06/final_cut_pro_x_backlash)). About seven months later, version 10.0.3 restored multicam, XML and more **without changing the magnetic model** ([Macworld](https://www.macworld.com/article/216378/first-look-final-cut-pro-x-10-0-3-restores-professional-features-adds-notable-new-ones.html)). Apple reported **over 2 million users** by April 2017 ([AppleInsider, 2017](https://appleinsider.com/articles/17/04/26/final-cut-pro-x-now-has-over-2-million-users-apple-says)), though high-end film editors largely never returned ([AppleInsider, 2025](https://appleinsider.com/articles/25/12/19/inside-final-cut-pro----apples-superb-video-editing-suite-and-a-huge-mistake)).

iWork's 2013 rewrite repeated the pattern. To unify the Mac, iOS and web versions, it dropped the customisable toolbar, AppleScript and more than 135 templates ([9to5Mac](https://9to5mac.com/2013/10/25/new-iwork-ilife-apps-go-for-simplicity-upset-power-users-all-over-again/)). Within about two weeks Apple announced that toolbar customisation, a vertical ruler, better alignment guides and AppleScript would return ([TechCrunch](https://techcrunch.com/2013/11/06/apple-addresses-iwork-for-mac-criticism-by-pre-announcing-the-return-of-many-features)). Our reading (inference) is that the constraints were never the problem. The damage came from taking away experts' tools and habits at the same moment, and customisation is what power users defend hardest.

What lasted across these redesigns is a set of techniques that work in place:

- **Keynote's inspector.** The Format inspector "shows formatting controls for whatever is selected". A separate Document inspector covers the whole file ([Apple Support, Keynote sidebars](https://support.apple.com/guide/keynote/show-or-hide-sidebars-tan391376b09/mac)).
- **Live alignment guides.** Keynote and Freeform show guides for centres, edges, equal size and equal spacing while you drag ([Apple Support, Keynote guides](https://support.apple.com/guide/keynote/use-alignment-guides-tan738df74cb/mac); [Apple Support, Freeform alignment](https://support.apple.com/en-gb/guide/freeform/frfma75f5f63/mac)).
- **Photos adjustments.** Each adjustment has one slider and an **Auto** button, and **Options** reveals the detailed sliders. A checkmark toggles a group of changes and a double-click resets a slider ([Apple Support, Photos](https://support.apple.com/guide/photos/adjust-light-exposure-and-color-pht806aea6a6/mac)).
- **Shortcuts parameters.** The essential parameters appear inline as a readable sentence and the rest go "under More Options". Some fields appear only when an earlier choice needs them ([WWDC19 213](https://developer.apple.com/videos/play/wwdc2019/213)).
- **Freeform Scenes.** On an infinite canvas, saved and named viewpoints were what reviewers called "by far, a better way to take new users through a detailed project" ([AppleInsider, 2024](https://appleinsider.com/articles/24/06/14/freeform-in-ios-18-is-enormously-easier-to-work-inside-and-navigate)).
- **Spotlight actions.** macOS Tahoe added actions and short, user-assigned **quick keys**, an expert layer that novices never see ([TechCrunch, 2025](https://techcrunch.com/2025/06/09/apple-updates-spotlight-to-take-actions-on-your-mac/)).

There is a counter-example to keep in mind. Jason Snell found that in Ventura's System Settings, search was "pretty much the only way to find anything" ([Six Colors](https://sixcolors.com/post/2022/08/ways-to-make-macos-ventura-system-settings-better/)). A command search cannot rescue a structure that novices can't browse.

Apple's consumer apps that deal with physical space show how to present it to non-experts:

- **Home** organises accessories by room and groups rooms into zones, in plain words ([Apple Support, Home zones](https://support.apple.com/guide/home/group-rooms-hme6660dd21e/mac)).
- **Maps** shows an indoor floor button such as "L1" only after you zoom in far enough for floors to matter ([Apple Support, Maps indoor](https://support.apple.com/guide/iphone/explore-airports-or-malls-iphd3705ff4e/ios)).
- **Find My's Precision Finding** guides people with direction, distance and haptics, and with VoiceOver speaks phrases like "AirTag is 9 feet away on your left" ([Apple Newsroom](https://www.apple.com/newsroom/2021/04/apple-introduces-airtag/)).

All of this has one gap: **Apple publishes no usability data** for any of these patterns. The evidence is adoption, reviews, forum reaction and Apple's own reversals.

## Independent studies say block errors, draw the shelf, and let people choose their layer

Research outside Apple supports the same approach and adds numbers.

**Preventing errors beats explaining them.** Carroll and Carrithers built a "Training Wheels" version of a word processor that made common error states unreachable. It produced "substantially faster learning". Control users, on the full system, spent almost a quarter of their time recovering from errors that the restricted version blocked ([Carroll & Carrithers, CACM 1984](https://dl.acm.org/doi/10.1145/358198.358218)). A follow-up found that training-wheels learners were afterwards *better* at advanced functions on the full system ([Catrambone & Carroll 1987](https://dl.acm.org/doi/10.1145/29933.275625)). Carroll's **Minimal Manual**, built around real tasks and recovering from errors, taught office temps who were new to computers. It cut learning time by **40%** and let them complete 2.7 times as many tasks ([Carroll et al. 1987](http://swcarpentry.github.io/swc-releases/2017.02/instructor-training/files/papers/carroll-minimal-manual-1987.pdf)).

**Plain language helps everyone, not only weak readers.** In NN/g's 2005 test with 50 users, a rewrite changed results like this ([NN/g](https://www.nngroup.com/articles/writing-for-lower-literacy-users/)):

| Users | Task success before → after | Task time before → after |
|---|---|---|
| Lower literacy | **46% → 82%** | 22.3 → 9.5 minutes |
| Higher literacy | **68% → 93%** | 14.3 → 5.1 minutes |

Related guidance:

- Icons need words beside them, because truly universal icons are rare ([NN/g, icon usability](https://www.nngroup.com/articles/icon-usability/)).
- GOV.UK puts one question on each page because "low-confidence users find them easier to use" ([GOV.UK design notes](https://designnotes.blog.gov.uk/2015/07/03/one-thing-per-page/)). Its service manual allows grouping questions for internal services "where government users need to repeat and switch between tasks quickly" ([GOV.UK Service Manual](https://www.gov.uk/service-manual/design/form-structure)).
- W3C's guide for users with cognitive disabilities includes "Let Users Go Back", "Make it Easy to Undo Form Errors" and "Do Not Rely on Users Calculations or Memorizing Information" ([W3C COGA](https://www.w3.org/TR/coga-usable/design_guide.html)).

**Layers work best when people or their administrators choose them, not when the interface adapts itself.**

- Shneiderman's multi-layer proposal starts novices on a layer that "permits safe exploration and therefore has no error messages". Users move up "when needed". He also records that "machine initiated changes to user interface features seem to be troubling to users" ([Shneiderman 2003](https://www.cs.umd.edu/~ben/ACM-CUU2003.pdf)).
- In Findlater and McGrenere's 27-person study, fixed menus were faster than menus that rearranged themselves. **55% preferred menus they could adjust themselves**, against 30% for self-adjusting and 15% for fixed ([Findlater 2004](https://www.cs.ubc.ca/labs/imager/th/2004/Findlater2004/Findlater2004.pdf)).
- Hiding has a measured cost. Users who started on a reduced interface found the core features faster, but were less *aware* of advanced features, which hurt them on new tasks ([Findlater & McGrenere, IJHCS 2010](https://dl.acm.org/doi/10.1016/j.ijhcs.2009.10.002)).
- NN/g adds that designs with more than **two disclosure levels** "typically have low usability" ([NN/g, progressive disclosure](https://www.nngroup.com/articles/progressive-disclosure/)).

**People do not become experts on their own.** Experienced users used keyboard shortcuts **less than 10% of the time** ([Lane et al. 2005](https://www.researchgate.net/publication/220302595_Hidden_Costs_of_Graphical_User_Interfaces_Failure_to_Make_the_Transition_from_Menus_and_Icon_Toolbars_to_Keyboard_Shortcuts)). Switching to a faster method causes a temporary "performance dip", and design has to soften it deliberately ([Scarr et al. 2011](https://www.csse.canterbury.ac.nz/andrew.cockburn/papers/blur.pdf)). Marking menus show the best-supported fix: make the beginner's route the same physical movement as the expert's, so using it is practice ([Kurtenbach thesis](https://www.research.autodesk.com/app/uploads/2023/03/the-design-and-evaluation.pdf_recHpUp1v9dc1n2CJ.pdf); [Buxton](https://www.billbuxton.com/MMUserLearn.html)). Alan Kay's slogan names the opposite risk: tools that do simple things "usually wall off next levels of complexity" ([Kay, quoted on Hacker News](https://news.ycombinator.com/item?id=24463842)). Resnick and Silverman add that "designs with well-chosen parameters are more successful than designs with fully-adjustable parameters" ([Resnick & Silverman 2005](https://web.media.mit.edu/~mres/papers/IDC-2005.pdf)).

**Warehouse evidence comes mostly from order picking. It is small but consistent.** In Guo et al.'s lab study (**n = 8**), each participant tried every method. A display showing a **picture of the shelving unit**, with rows coded by colour and columns by symbol, beat a paper pick list ([Guo et al. 2014](https://guoanhong.com/papers/ISWC14-OrderPicking.pdf)):

| Method | Errors per pick | Time per task |
|---|---|---|
| Shelf picture (head-up display) | **0.006** | **38.6 s** |
| Paper list | 0.030 | 62.3 s |

The picture also gave lower workload. Paper failed because "there is not a natural mapping from the paper pick lists to where to pick". Pick-by-light removed wrong-bin errors, but participants missed items at the edges of the shelf and "did not even notice". Earlier work cited in the paper found that adding part images "slowed the user by giving them too much to look at" (same source).

Two other findings bear on the floor:

- **Colour.** Process-control displays built to the ISA-101 standard use a muted grey base and keep colour for abnormal states. An industry consortium reports that operators using such displays spotted events before the alarm 48% of the time, against 10% on a traditional display. That figure comes from interested parties, not peer review ([Chemical Processing](https://www.chemicalprocessing.com/automation/automation-it/article/11376291/process-engineering-asm-outperforms-traditional-interface-chemical-processing)).
- **Gloves.** The only guidance found is qualitative. Touchscreens need a "large button… for finger use", and "gloved operation may be incompatible with some touch technology" ([US NRC 2025](https://www.nrc.gov/docs/ML2522/ML25226A197.pdf)). WCAG 2.2 sets a minimum target size of 24 × 24 CSS px ([W3C 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)).

**Research on how people read maps decides which view to show for which job.** Across six experiments, St. John and colleagues found that "the distortions inherent in 3D displays hamper judging relative positions, whereas the integration of dimensions in 3D displays facilitates shape understanding" ([St. John et al. 2001](https://journals.sagepub.com/doi/abs/10.1518/001872001775992534)). In short: **use 2D for "which bay, which level" and 3D for "what does this area look like"**. Other findings:

- Maps that are not aligned with the way the viewer is facing cause predictable mirror-image and alignment errors ([Levine 1982](https://journals.sagepub.com/doi/abs/10.1177/0013916584142006); [Rossano & Warren 1989](https://doi.org/10.1068/p180215)).
- Maps with a fixed orientation help people plan and build a mental map. Maps that rotate with the traveller help with the next turn ([Aretz & Wickens 1992](https://www.tandfonline.com/doi/abs/10.1207/s15327108ijap0101_2)).
- Turn-by-turn guidance gets people there but teaches them the layout worse than a map does ([Ishikawa et al. 2008](https://www.semanticscholar.org/paper/Wayfinding-with-a-GPS-based-mobile-navigation-A-and-Ishikawa-Fujiwara/35eb6ed7855d4ece2c0836bf2c7d13438b1ad2ba)).
- Good directions name landmarks at decision points and say what "gone too far" looks like ([Lovelace et al. 1999](https://www.amlap.org/~masta/WS13/Lovelace_etal_goodDirections_99.pdf)).

The caveats are real. The picking studies are small lab studies, and the learning studies used 1980s office software. **No study was found that tests low-digital-literacy warehouse workers using layout or warehouse-management software.**

## Layout editors succeed by drawing shape first, and no phone can measure a warehouse

The floor planners that beginners handle well share one habit: get the shape first, and the numbers later or never.

- magicplan has people tap corners on a grid of 1 m² tiles, using the grid "to estimate the length and angle", and only afterwards "adjust the dimensions" ([magicplan Help](https://help.magicplan.app/create-a-room-with-the-define-corners-feature)).
- Floorplanner's Draw Room stretches four walls from a corner in one drag ([Floorplanner Help](https://help.floorplanner.com/en/articles/8426766-how-to-draw-a-wall-and-how-to-stop-drawing)).
- IKEA's wardrobe planner opens on "a square, generic space" rather than an empty canvas ([user walkthrough](https://chrislovesjulia.com/how-to-use-the-ikea-pax-wardrobe-planner-our-master-closet-mood-board/)).
- Sweet Home 3D snaps walls to 15° steps by default, and a held key turns snapping off. Double-clicking inside closed walls creates a room, and the 3D view updates with every 2D change ([Sweet Home 3D User's Guide](https://www.sweethome3d.com/users-guide/)). Its users have asked for a visible on/off button for snapping, not only a held key ([SH3D forum](https://www.sweethome3d.com/support/forum/viewthread_thread,4159)).

The failures are just as consistent. Floorplanner needed a help article on how to *stop* drawing a chain of walls ([Floorplanner Help](https://help.floorplanner.com/en/articles/8426766-how-to-draw-a-wall-and-how-to-stop-drawing)). RoomSketcher added an "Always Furniture" setting because in its separate modes, new walls could "crash" into furniture that was hidden ([RoomSketcher blog](https://www.roomsketcher.com/blog/roomsketcher-drawing-tips-tricks/)). NN/g notes that modes which last only while a key is held prevent mode mistakes but are harder to discover ([NN/g, modes](https://www.nngroup.com/articles/modes/)).

Design tools and game builders show how to repeat things and how to be precise without numbers:

- **Repeating.** Adobe XD's Repeat Grid copies an element as you drag a handle. PowerPoint's Ctrl+D "remembers the offset of your last manual move". In Figma, editing the main component updates every linked copy. (All three come from search summaries: [Adobe XD Help](https://helpx.adobe.com/xd/help/create-repeating-elements.html); [Nuts & Bolts](https://nutsandboltsspeedtraining.com/powerpoint-tutorials/ctrl-d-powerpoint/); [Figma Learn](https://help.figma.com/hc/en-us/articles/360038662654-Guide-to-components-in-Figma).)
- **Limits on copies.** A Figma copy cannot change its layout direction, which shows that what a copy may change has to be decided on purpose ([Figma Forum](https://forum.figma.com/suggest-a-feature-11/override-auto-layout-direction-on-the-instances-24618)).
- **Snapping.** Bier and Stone's 1986 snap-dragging gives precision through temporary alignment lines instead of typed coordinates ([dblp: SIGGRAPH 1986](https://dblp.org/db/conf/siggraph/siggraph1986.html)).
- **Sketchy style.** A hand-drawn look signals that a plan is provisional ([Isenberg et al. 2006, summarising Schumann et al.](https://ires.cpsc.ucalgary.ca/publ/papers/2006/refs/Isenberg%20et%20al.%20'06.pdf)). That suits a layout whose positions are approximations.
- **Contents move with their container.** The Sims lets players drag rooms whose contents move with the walls. Furniture that no longer fits goes to the household inventory instead of being deleted ([Sims Wiki, search summary](https://sims.fandom.com/wiki/Build_mode_%28The_Sims_4%29)).
- **Finer control for experts.** Roblox Studio lets expert builders change the snap step ([Roblox DevForum](https://devforum.roblox.com/t/how-do-you-change-the-increments-on-roblox-studio/537426)).

Novices also think in relative terms. In a 22-person study of 3D modelling by voice, "novices often use relative references" such as "place the second box under the first" ([Desolda et al.](https://arxiv.org/pdf/2307.04481)).

**Warehouse software mostly skips drawing.** ERP and warehouse-management products generate bin *codes* by combining number ranges for each part of the code:

- Dynamics 365's wizard turns ranges of 1–3 on four parts into 81 locations ([Microsoft Learn, D365](https://learn.microsoft.com/en-us/dynamics365/supply-chain/warehousing/tasks/configure-locations-wms-enabled-warehouse)).
- Business Central cannot generate letter sequences, and handles a passageway by deleting the bins it generated there. It will not delete a bin that has been used, so renaming means moving stock ([Microsoft Learn, Business Central](https://learn.microsoft.com/en-us/dynamics365/business-central/warehouse-how-to-create-individual-bins)).
- SAP EWM shows how many bins a template will make, and offers a simulation before saving ([SastraGeek](https://www.sastrageek.com/post/ewm-storage-bins-creation)).

Where maps exist, they are usually generated from stored coordinates, as in SAP's Graphical Warehouse Layout ([SAP Help](https://help.sap.com/doc/saphelp_ewm900/9.0/en-US/1a/5ec67882d4481b9f47fbe6c609641d/content.htm?no_cache=true)). Some show a front view of each rack plus a top-down plan, as LOGIA does ([Roimaint](https://www.roimaint.com/en/content/logia-wms-3d-visualization)). One small open-source project builds the map from bin addresses and lets owners "drag a bay on the floor plan so the diagram matches the real building" ([project-rackline](https://github.com/brianventr/project-rackline/pull/3)). Newer vendors pitch drawing your own layout "instead of depending on an IT consultant" ([PULPO WMS](https://www.pulpowms.com/warehouse-wizard)), but **none publishes evidence that floor staff use these editors**. A relative, drawn layout seeded from the bin list fills a real gap between code lists and measured CAD drawings (inference).

Scanning with a phone does not fill that gap:

| RoomPlan limit | Value |
|---|---|
| Recommended room size | about **9 m × 9 m** |
| Largest room it is designed for | **15 m × 15 m**, **3.6 m** high |
| Object categories | 16 household categories; no rack or shelving |
| Several rooms merged into one model | about **186 m²** of single-floor housing at most |
| LiDAR sensor range | about **5 m** |

Sources: [WWDC22 10127](https://developer.apple.com/videos/play/wwdc2022/10127/); [Apple ML Research](https://machinelearning.apple.com/research/roomplan); [WWDC23 10192](https://developer.apple.com/videos/play/wwdc2023/10192/); [Apple press release via Business Wire](https://www.businesswire.com/news/home/20200318005341/en).

In 2025 a developer asked on Apple's forums whether RoomPlan had been abandoned, after two years without updates ([Apple Developer Forums](https://developer.apple.com/forums/thread/787628)). Warehouses are eligible for Apple's own Indoor Maps programme ([Apple Indoor Maps Program](https://register.apple.com/resources/indoor/program/)), but it starts from existing CAD, BIM or GIS drawings. Staff then walk the floor and "drop pins every five to eight meters" only to calibrate positioning ([WWDC19 245](https://developer.apple.com/videos/play/wwdc2019/245/)).

Apple's indoor map format, IMDF, is still a useful check on structure ([OGC IMDF](https://docs.ogc.org/cs/20-094/index.html)):

- It stacks floors by a whole-number `ordinal` ([Level](https://docs.ogc.org/cs/20-094/Level/index.html)).
- Touching fixtures of the same kind "SHOULD be merged" ([Fixture](https://docs.ogc.org/cs/20-094/Fixture/index.html)).
- It has no rack category and no height attribute ([Categories](https://docs.ogc.org/cs/20-094/Categories/index.html)).

Spork's boxes-inside-boxes model already has this shape. A rack run is one place with a grid, not one object per bay, which matches the merge rule (inference).

## Recommendation: one place model, three depths, no modes

### The paradigm: exact facts come from the floor, geometry is drawn at the desk

We recommend that Spork treat the layout as **one place model, seen at three depths, with the depth set by role and no global mode switch**:

| Depth | Who | What it covers |
|---|---|---|
| 1. **Find and check** | Everyone | The layout appears as place pages, reached by a scan, that show where a bin is as a picture of the shelf. |
| 2. **Put it right** | Trusted floor roles | Correcting which bin sits in which cell by scanning the bin where it physically is. |
| 3. **Shape the layout** | Office users with edit rights | The desktop editor: generating, drawing, arranging, adding grids and naming. |

Every depth uses the same nouns (the place's own name, bay, level, bin) and the same objects. Moving up a depth means learning more actions, not a new model. Kay, Shneiderman and the training-wheels transfer result all ask for this, and Logic's removal of its switch points the same way (the link is our inference). Depth comes from the roles Spork already has. It is never inferred from behaviour, and the next depth is always signposted so that hiding it does not make people unaware of it.

This split follows from Spork's data model. Relationships are exact and positions are approximate. The fact that must be right, which bin is in which cell, is confirmed with a skill floor staff already have: scanning a label at the shelf. The hard part, geometry, is edited only at a desk with a mouse, snapping and undo, where being slightly wrong does no harm. This is our inference, grounded in Tesler's conservation of complexity and the training-wheels evidence.

In the tables below, **Documented** cites what a source shows. **Inference** is our reasoning about why it applies to Spork; no source has tested it there. Priority 1 items should ship before floor staff see the feature. Priority 2 items make it pleasant to use. Priority 3 items are refinements.

### Depths 1 and 2: on the floor, the layout is a picture of the shelf

The worker's main screen is the **place page**. It opens when a worker scans a bin label on the handheld, or scans or types a code into the desktop search box. It shows the place that holds the bin, with the bin selected. From top to bottom:

1. **Breadcrumb.** For example *Building › Mezzanine › Rack C*, shown as a row of large chips. Tapping a chip goes up to that level. This answers Stern's "where am I… how do I get out" ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)).
2. **Face grid.** The front of the place: bays across and levels up. The target cell is filled with the accent colour and shows the bin code in large type. Bay numbers run along the bottom and level labels up the side, written exactly as the physical labels read. If the place has no grid (for example, a staging area that is a single bin), the plan with that area lit is the picture instead.
3. **Plan thumbnail.** A small plan that always faces the same way, with the place outlined. Tapping it opens the plan full screen. Nothing requires panning or pinching.
4. **See in 3D.** A secondary button that opens the 3D view to help with orientation.
5. **Something's wrong here.** The one primary action: a full-width button at the bottom, within reach of the thumb.

**Browsing without a scan.** A **Places** list drills down by name (site, then building, room, rack) in big rows, the way Home drills from zone to room. At the top, views that office users have saved and named ("Mezzanine", "Dock end") appear as big buttons. A floor switch appears only inside a building that contains a mezzanine, the way Maps shows "L1" only when floors matter. Each place shows its free-text name and an icon taken from what the place is in the model, since there is no separate "kind" field:

- a place with a grid shows a rack icon;
- a walk-through place shows a floor icon;
- a solid place without a grid shows a block icon.

**The Something's wrong here flow** puts one question on each screen:

1. The first screen offers three choices, each with a picture: *A bin isn't where this says*, *There's a bin here that isn't shown*, and *I can't get to this spot*.
2. For the two bin choices, the worker scans the bin and taps the cell it is really in.
3. A check screen reads the change back in plain words: "Move C-07-3 from bay 07, level 3 to bay 08, level 3?"
4. **Save** returns to the place page with a banner offering **Undo**.

Depth-2 roles apply the change directly. For depth-1 roles, the same answers become a flag that appears on the office plan.

**Check this rack** (priority 3) lets a worker scan labels along a rack in any order. Each scan ticks its cell, and cells that disagree are flagged.

| # | Priority | Principle and what to build | Documented | Inference |
|---|---|---|---|---|
| W1 | 1 | **A scan opens a picture of the shelf, not a code.** The place page: breadcrumb chips, face grid with the target lit, labels as printed on the rack, a plan thumbnail that always faces the same way. Say bay and level, never "left" or "right". | A shelf picture gave 0.006 errors per pick vs 0.030 on paper, and 38.6 s vs 62.3 s; paper lacks a "natural mapping" ([Guo 2014](https://guoanhong.com/papers/ISWC14-OrderPicking.pdf)). "Do Not Rely on… Memorizing" ([W3C COGA](https://www.w3.org/TR/coga-usable/design_guide.html)). Every screen should answer "where am I" ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)). Misaligned maps cause mirror-image errors ([Levine 1982](https://journals.sagepub.com/doi/abs/10.1177/0013916584142006)). Fixed-orientation maps help build a mental map ([Aretz & Wickens 1992](https://www.tandfonline.com/doi/abs/10.1207/s15327108ijap0101_2)). | The face grid is Spork's version of Guo's shelf picture, but results from 8 students in a lab will not carry over exactly. "Left" and "right" depend on which way the worker approaches, and the model does not record that. |
| W2 | 1 | **2D answers "where"; 3D is on request.** The handheld never opens in 3D. *See in 3D* is a secondary button. | 3D makes relative positions harder to judge and shape easier to grasp ([St. John 2001](https://journals.sagepub.com/doi/abs/10.1518/001872001775992534)). Apple advises starting spatial apps in a familiar window and saving depth for key moments ([WWDC23 10072](https://developer.apple.com/videos/play/wwdc2023/10072/)). | 3D earns its place for getting your bearings in an unfamiliar building, not for finding a bin. |
| W3 | 1 | **Nothing on the handheld edits geometry or destroys data.** No dragging, rotating or deleting places. Every save offers Undo. Inside a flow, choices that aren't allowed (for example, a cell that can't hold a bin) are greyed out rather than answered with an error message. | Blocking error states sped learning; control users spent nearly a quarter of their time recovering ([Carroll & Carrithers 1984](https://dl.acm.org/doi/10.1145/358198.358218)). The first layer "has no error messages" ([Shneiderman 2003](https://www.cs.umd.edu/~ben/ACM-CUU2003.pdf)). Gloves and imprecise pointing on touchscreens ([US NRC 2025](https://www.nrc.gov/docs/ML2522/ML25226A197.pdf)). Unavailable actions are "merely dimmed" ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). | Positions are approximations, so nudging them with gloves on gains little and risks a lot. |
| W4 | 1 | **Plain words, icon plus word, one question per screen, one big action at the bottom.** Aim for a sixth-grade reading level. No hover. Scan or pick from a list instead of typing. | A plain-language rewrite lifted success from 46% to 82% (lower literacy) and 68% to 93% (higher) ([NN/g 2005](https://www.nngroup.com/articles/writing-for-lower-literacy-users/)). Icons need labels ([NN/g](https://www.nngroup.com/articles/icon-usability/)). One question per page suits low-confidence users ([GOV.UK](https://designnotes.blog.gov.uk/2015/07/03/one-thing-per-page/)). "Plain, direct language, avoiding metaphors" ([WWDC21 10275](https://developer.apple.com/videos/play/wwdc2021/10275/)). | Spork's 48 px targets are already double the 24 px WCAG minimum ([W3C](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)). For gloves, make the primary action full width. |
| W5 | 2 | **Workers correct the layout by scanning where things are.** The *Something's wrong here* flow. Depth 2 applies the change; depth 1 raises a flag. | Confirming at the point of action reduces errors, and pick-by-light without confirmation hid misses ([Guo 2014](https://guoanhong.com/papers/ISWC14-OrderPicking.pdf)). GOV.UK ends forms with a "check your answers" page ([GOV.UK](https://designnotes.blog.gov.uk/2015/07/03/one-thing-per-page/)). A vendor's warehouse model flags location mismatches by colour ([Dexory, vendor](https://www.dexory.com/insights/enhancing-operations-and-efficiency-with-warehouse-digital-twins)). | Because relationships are exact, a scan at the shelf is the ground truth, and it uses a skill workers already have. |
| W6 | 2 | **Navigate by name as well as by map.** The Places list, named views saved by office users, and a floor switch only when there is a mezzanine. | Home's rooms and zones ([Apple Support](https://support.apple.com/guide/home/group-rooms-hme6660dd21e/mac)). Maps' floor button appears when zoomed in ([Apple Support](https://support.apple.com/guide/iphone/explore-airports-or-malls-iphd3705ff4e/ios)). Freeform's saved Scenes helped new users ([AppleInsider](https://appleinsider.com/articles/24/06/14/freeform-in-ios-18-is-enormously-easier-to-work-inside-and-navigate)). Search alone failed in Ventura's Settings ([Six Colors](https://sixcolors.com/post/2022/08/ways-to-make-macos-ventura-system-settings-better/)). | Panning and zooming are skills; reading a list is not. |
| W7 | 2 | **Colour means "look here".** Grey plan and grid, one accent colour for the target, and one alert style (colour + icon + word) for flags and unplaced bins. Copy the colours of physical level labels only if the racks actually have them. | Colour kept for abnormal states ([RealPars on ISA-101](https://www.realpars.com/blog/high-performance-hmi); industry-reported study, [Chemical Processing](https://www.chemicalprocessing.com/automation/automation-it/article/11376291/process-engineering-asm-outperforms-traditional-interface-chemical-processing)). Colour-coded rows helped pickers ([Guo 2014](https://guoanhong.com/papers/ISWC14-OrderPicking.pdf)). | This resolves the conflict between the two sources: colour on screen should either match colour on the racks or signal a problem. |
| W8 | 3 | **Check a rack by walking it.** Scan labels in any order; cells tick; mismatches are flagged; the rack records who checked it and when. | Apple's indoor survey has staff walk the floor and drop pins every 5–8 m ([WWDC19 245](https://developer.apple.com/videos/play/wwdc2019/245/)). Scans reconciled against the warehouse system ([Dexory, vendor](https://www.dexory.com/insights/enhancing-operations-and-efficiency-with-warehouse-digital-twins)). | Turns a layout audit into a scanning task. Untested. |
| W9 | 3 | **Teach in place, in seconds.** One tip beside the face grid on first use ("The lit square is where the bin is") and a 15-second *Show me* clip for each task. No tutorial wizard. | Apple prefers context-specific tips over a single onboarding flow ([HIG: Onboarding](https://developer.apple.com/design/human-interface-guidelines/onboarding)). Shneiderman used 5–30 s *Show me* demos ([Shneiderman 2003](https://www.cs.umd.edu/~ben/ACM-CUU2003.pdf)). Minimal, task-focused instruction cut learning time by 40% ([Carroll 1987](http://swcarpentry.github.io/swc-releases/2017.02/instructor-training/files/papers/carroll-minimal-manual-1987.pdf)). | With frequent new hires, every minute of training saved is saved again for each one. |

### Depth 3: the desktop editor opens on a draft and keeps expert tools one step away

**Page structure.** The **Layout** page sits in the existing desktop shell, with the usual sidebar and header. From top to bottom and left to right:

- the page header, showing the site and a breadcrumb of the place being edited;
- a toolbar of no more than three groups, **Add**, **Arrange** and **View**, grouped "by function and frequency" ([WWDC25 356](https://developer.apple.com/videos/play/wwdc2025/356/));
- the plan canvas in the middle;
- the 3D pane beside the canvas;
- the inspector on the far right.

**Editing inside one place at a time.** The canvas always shows the inside of one place, the building by default. Its children are drawn as boxes and outlines. Double-clicking a child opens it, and the breadcrumb grows. Escape or a breadcrumb click goes back up, and the parent's surroundings stay faintly visible for context. This makes the boxes-inside-boxes model easy to move around and keeps the number of things on screen small at every level.

**Never an empty canvas.** A new site opens on one primary button, **Draft from bin list**. It works in two steps:

1. **Preview.** It groups the imported bin codes by shared prefix and proposes one place with a grid for each group, sized from the bay and level ranges it finds. Before anything is created, it shows a preview: how many places, how many bins land in cells, and how many fit no pattern.
2. **Create.** The draft places appear in a tidy row, in code order, inside a rectangular building. Someone who knows the floor then drags them roughly to where they really stand.

That person may be a floor lead sitting beside an office user. GOV.UK's guidance for people who cannot use a digital service on their own expects that they may need someone beside them, and asks that the help leave them better able to do it alone next time ([GOV.UK Service Manual](https://www.gov.uk/service-manual/helping-people-to-use-your-service/assisted-digital-support-introduction)). Bins that fit no cell wait in an **Unplaced** tray docked at the bottom of the canvas. The tray is visible whenever it is not empty, and its bins can be dragged onto cells.

**Drawing is dragging.** **Add box** and **Add outline** create a rectangle by dragging. To make an L-shape or any other outline, drag the midpoint handle of an edge to add a corner. There is no mode where you click point by point. Snapping and sizing work like this:

- Everything snaps to whole cells of the parent and to quarter turns.
- Live guides show matching centres and edges, equal sizes and equal gaps.
- Holding **Alt** turns snapping off (for example, to set a free angle) for as long as the key is held. A visible **Snap** toggle in the View group does the same for people who don't know the key.
- Sizes are shown in cells ("30 × 5"), never in metres. The drawing style is plain and schematic, so nobody mistakes the plan for a survey.

**Constraints catch the errors instead of error messages:**

- A child cannot be dragged outside its parent.
- A solid place pushed into another solid place stops flush against it. If forced, it shows a not-allowed outline.
- Walk-through places may overlap other places.
- Moving a place carries its children and bins with it.
- Deleting a place moves its bins to the Unplaced tray. Nothing is deleted, because the bins come from the ERP.

**The inspector follows the selection:**

- **Nothing selected:** it shows the current place, like Keynote's Document inspector.
- **One place selected:** it shows, in order:
  1. **Name**.
  2. A **Walk-through / Solid** switch.
  3. The **Grid** (bays × levels), or an **Add grid** button when there is none.
  4. The **Naming pattern**, written as a sentence with its placeholders shown as chips, such as *Name bins C-{bay:02}-{level}*. Beneath it a live line reads, for example, "C-01-1 to C-30-5 · 148 of 150 found · 2 missing", and missing cells are hatched on the face grid.
- **One More options disclosure** holds the exact angle, first bay and level numbers, numbering step (2 for odd/even sides, −1 to count down), level lettering and numbering direction.
- **Several places selected:** it shows the fields they share, plus Align and Distribute.

**Preview before bins move.** Some changes would move bins between cells: editing a pattern, shrinking a grid, deleting a place. These show a preview first ("12 bins move, 3 become unplaced"), matching the dry-run step Spork's imports already use. Changes that only move places on the plan apply immediately. Undo is unlimited, named for what it undoes ("Undo Move Rack C"), and pans the view to show the result.

**Shortcuts duplicate visible commands:**

- A **Layout** menu in the page header lists every command with its shortcut.
- Right-clicking a place offers the five to eight most common commands: Open, Rename, Duplicate, Rotate ¼, Add grid, Delete.
- Arrow keys nudge a selected place by one cell.
- The existing search box already takes barcode scans. Here it jumps to any place or bin (scan a label at the desk and the editor selects its cell), and it runs commands typed with the same words as the buttons.
- Dragging the repeat handle on a selected place extends it into a run.
- **Duplicate** remembers the offset of the last manual move.
- Duplicating a place whose pattern starts with a letter suggests the next letter (C to D) and shows how many bins that would match before committing.

**The 3D pane** sits beside the plan and updates live. It shares the selection, so a place selected in either view is highlighted in both. It offers orbit and zoom only; the editing handles live in the 2D plan. A button or a shortcut collapses it. Solid places are drawn as blocks and walk-through places as floor.

| # | Priority | Principle and what to build | Documented | Inference |
|---|---|---|---|---|
| P1 | 1 | **One editor for everyone; depth set by role, never by a mode switch.** No "Advanced mode" setting. Editing tools appear with edit permission. On the handheld, roles that can edit see a quiet line: "Edit the layout on a computer". | Logic removed its switch in Logic 12 ([Apple Support](https://support.apple.com/guide/logicpro/whats-new-in-logic-pro-lgcp4a62a494/mac)). 55% preferred menus they could adjust themselves ([Findlater 2004](https://www.cs.ubc.ca/labs/imager/th/2004/Findlater2004/Findlater2004.pdf)). Self-adjusting interfaces are "troubling" ([Shneiderman 2003](https://www.cs.umd.edu/~ben/ACM-CUU2003.pdf)). Hiding features reduces awareness of them ([Findlater & McGrenere 2010](https://dl.acm.org/doi/10.1016/j.ijhcs.2009.10.002)). | Logic's reversal and the awareness cost both argue for signposting the next depth rather than hiding it completely. |
| P2 | 1 | **Open on a draft from the bin list, never on a blank canvas.** Show counts before creating. Keep an Unplaced tray. | SAP shows the bin count and a simulation before saving ([SastraGeek](https://www.sastrageek.com/post/ewm-storage-bins-creation)). Business Central lets you review generated bins before creating them ([Microsoft Learn](https://learn.microsoft.com/en-us/dynamics365/business-central/warehouse-how-to-create-individual-bins)). Planners start from a rough shape ([magicplan](https://help.magicplan.app/create-a-room-with-the-define-corners-feature)). A map built from addresses, then dragged to match ([project-rackline](https://github.com/brianventr/project-rackline/pull/3)). The Sims sends furniture that no longer fits to inventory ([Sims Wiki](https://sims.fandom.com/wiki/Build_mode_%28The_Sims_4%29)). | Arranging racks that already exist is easier than drawing from nothing. The claim that blank canvases make people give up comes only from a vendor blog ([Userpilot](https://userpilot.com/blog/best-user-onboarding-experience/)). |
| P3 | 1 | **Edit inside one place at a time, with a breadcrumb out.** Double-click to open a place; Escape to go up. | Every screen should say where you are and how to get out ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)). Home's nested rooms and zones ([Apple Support](https://support.apple.com/guide/home/group-rooms-hme6660dd21e/mac)). Sweet Home 3D shows lower levels faded ([SH3D Guide](https://www.sweethome3d.com/users-guide/)). | Matches boxes inside boxes and keeps each view small. |
| P4 | 1 | **Constraints instead of error reports, and a held key to override them.** Children stay inside parents; solid places stop flush; a parent carries its children and bins; delete sends bins to Unplaced; Alt turns snapping off, and there is also a visible Snap toggle. | The magnetic timeline prevents gaps and collisions, and the Position tool leaves a gap clip ([Apple Support](https://support.apple.com/guide/final-cut-pro/intro-to-the-magnetic-timeline-verb8fcfc133/mac)). Training wheels ([Carroll & Carrithers 1984](https://dl.acm.org/doi/10.1145/358198.358218)). Modes that last only while a key is held are acceptable ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). Users asked for a visible snapping toggle ([SH3D forum](https://www.sweethome3d.com/support/forum/viewthread_thread,4159)). Show whether a drop target will accept the item ([HIG: Drag and drop](https://developer.apple.com/design/human-interface-guidelines/drag-and-drop)). | The Unplaced tray is Spork's gap clip: nothing disappears without a trace. |
| P5 | 1 | **An inspector that follows the selection, with one "More options".** Show only the fields that apply: no grid fields until the place has a grid. | Keynote's Format and Document inspectors ([Apple Support](https://support.apple.com/guide/keynote/show-or-hide-sidebars-tan391376b09/mac)). One labelled disclosure per view ([HIG](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)). More than two levels loses users ([NN/g](https://www.nngroup.com/articles/progressive-disclosure/)). In Logic, a project turned on the features it used ([Apple Support](https://support.apple.com/kb/PH24704?locale=en_US&viewlocale=en_US)). | Showing grid fields only once a grid exists is Logic's "the project decides" on a small scale. |
| P6 | 1 | **Unlimited named undo; preview anything that moves bins between cells.** | Undo should be unlimited, named, and show its result ([HIG: Undo and redo](https://developer.apple.com/design/human-interface-guidelines/undo-and-redo)). SAP's simulation before saving ([SastraGeek](https://www.sastrageek.com/post/ewm-storage-bins-creation)). Renaming a used bin means moving stock ([Microsoft Learn](https://learn.microsoft.com/en-us/dynamics365/business-central/warehouse-how-to-create-individual-bins)). | Which bin is in which cell is exact and drives work on the floor, so changes to it deserve a preview. Positions don't. |
| P7 | 2 | **The naming pattern reads as a sentence, with live match counts.** Placeholders as chips; start numbers, step and lettering under More options. | Shortcuts shows key settings as a sentence with the rest under More Options ([WWDC19 213](https://developer.apple.com/videos/play/wwdc2019/213)). ERP generators cannot do letter sequences and limit code length ([Business Central](https://learn.microsoft.com/en-us/dynamics365/business-central/warehouse-how-to-create-individual-bins); [D365](https://learn.microsoft.com/en-us/dynamics365/supply-chain/warehousing/tasks/configure-locations-wms-enabled-warehouse)). Feedback should answer "what will happen" ([WWDC17 802](https://developer.apple.com/videos/play/wwdc2017/802/)). | Live counts turn pattern mistakes into hatched cells you can see, instead of lines in a later report. |
| P8 | 2 | **Precision without numbers.** Snap to cells and quarter turns, live guides, sizes shown in cells, a schematic drawing style. | Keynote and Freeform guides ([Keynote](https://support.apple.com/guide/keynote/use-alignment-guides-tan738df74cb/mac); [Freeform](https://support.apple.com/en-gb/guide/freeform/frfma75f5f63/mac)). Sweet Home 3D snaps walls to 15° ([SH3D](https://www.sweethome3d.com/users-guide/)). A sketchy style signals a provisional plan ([Isenberg et al.](https://ires.cpsc.ucalgary.ca/publ/papers/2006/refs/Isenberg%20et%20al.%20'06.pdf)). | Showing metres would invite measuring, which the model deliberately does not do. |
| P9 | 2 | **Draw by dragging, with no click-by-click drawing mode.** Drag out a rectangle, add corners by dragging an edge's midpoint, and Escape always ends. | Floorplanner needed a help article on how to stop drawing ([Floorplanner Help](https://help.floorplanner.com/en/articles/8426766-how-to-draw-a-wall-and-how-to-stop-drawing)). People forget which mode they are in ([NN/g](https://www.nngroup.com/articles/modes/)). magicplan adds corners to an existing wall ([magicplan](https://help.magicplan.app/create-a-room-with-the-define-corners-feature)). | An L-shaped room becomes an edit to a rectangle rather than a drawing skill. |
| P10 | 2 | **Make one, repeat it.** A repeat handle; Duplicate remembers the offset; duplicating suggests the next letter, with a preview. | Repeat Grid, Ctrl+D's remembered offset, and components (search summaries: [Adobe](https://helpx.adobe.com/xd/help/create-repeating-elements.html); [Nuts & Bolts](https://nutsandboltsspeedtraining.com/powerpoint-tutorials/ctrl-d-powerpoint/); [Figma](https://help.figma.com/hc/en-us/articles/360038662654-Guide-to-components-in-Figma)). "What they meant rather than what they did" ([Commoncog](https://commoncog.com/creative-selection/)). | Most warehouses are runs of similar racks, so repetition is the biggest time saver for experts. |
| P11 | 2 | **Every command has a visible home; shortcuts and search only duplicate it.** A Layout menu showing shortcuts; a right-click menu of 5–8 items; the search box jumps to places and runs commands using the button words; few custom shortcuts, standard ones kept. | Menus list every command with its shortcut ([HIG: Menus](https://developer.apple.com/design/human-interface-guidelines/menus)). Context-menu items must also appear in the main interface ([HIG](https://developer.apple.com/design/human-interface-guidelines/context-menus)). Keep custom shortcuts few ([HIG: Keyboards](https://developer.apple.com/design/human-interface-guidelines/keyboards)). Spotlight quick keys ([TechCrunch](https://techcrunch.com/2025/06/09/apple-updates-spotlight-to-take-actions-on-your-mac/)). Shortcuts used less than 10% of the time ([Lane 2005](https://www.researchgate.net/publication/220302595_Hidden_Costs_of_Graphical_User_Interfaces_Failure_to_Make_the_Transition_from_Menus_and_Icon_Toolbars_to_Keyboard_Shortcuts)). Beginner routes that double as practice ([Kurtenbach](https://www.research.autodesk.com/app/uploads/2023/03/the-design-and-evaluation.pdf_recHpUp1v9dc1n2CJ.pdf)). | Scanning a label into the desktop search is the same action as on the floor, so there is nothing new to learn. |
| P12 | 2 | **2D edits, 3D confirms.** Live, beside the plan, collapsible, shared selection, orbit and zoom only. | Sweet Home 3D updates 3D with every 2D change ([SH3D](https://www.sweethome3d.com/users-guide/)). Let people hide a pane and reveal it with a button or shortcut; highlight the selection in each pane ([HIG: Split views](https://developer.apple.com/design/human-interface-guidelines/split-views)). 2D is better for relative positions ([St. John 2001](https://journals.sagepub.com/doi/abs/10.1518/001872001775992534)). Novices asked for a top-down view to fix positions ([Desolda](https://arxiv.org/pdf/2307.04481)). | Editing in 3D is where novices lose their bearings. |
| P13 | 3 | **A customisable toolbar and shared named views.** | Customisation suits long sessions ([HIG: Toolbars](https://developer.apple.com/design/human-interface-guidelines/toolbars)). Toolbar customisation came back after the 2013 iWork backlash ([TechCrunch](https://techcrunch.com/2013/11/06/apple-addresses-iwork-for-mac-criticism-by-pre-announcing-the-return-of-many-features)). Freeform Scenes ([AppleInsider](https://appleinsider.com/articles/24/06/14/freeform-in-ios-18-is-enormously-easier-to-work-inside-and-navigate)). | Named views also become the handheld's big buttons (W6). |
| P14 | 3 | **Show what changed.** A before-and-after review animates places that moved and fades in or out places that were added or removed. | Keynote's Magic Move builds an animation from two versions of a slide ([Apple Support](https://support.apple.com/guide/keynote/add-transitions-tanff5ae749e/mac)). | Helps a supervisor explain a change of layout to the floor. |

### What lives where

| Capability | Handheld (depths 1–2) | Desktop, visible by default | Desktop, one step in | Other routes to the same thing |
|---|---|---|---|---|
| Find a bin or place | Scan, or the Places list | Search box (scan or type) | — | Scanning at the desk selects the bin's cell |
| See where it is | Place page: face grid and plan thumbnail | Canvas and inspector | — | — |
| 3D | *See in 3D* button | Live pane beside the plan | — | Shortcut shows or hides the pane |
| Report a problem | *Something's wrong here* (depth 1 raises a flag) | Flags on the plan in the alert style | List of flags | — |
| Move a bin to another cell | Scan-at-the-shelf flow (depth 2) | Drag on the face grid | — | Cut and paste |
| Move or rotate a place | Not available | Drag; rotate handle snaps to quarter turns | Exact angle under More options | Arrow keys; hold Alt for a free angle |
| Add places | Not available | Add box, Add outline | — | Repeat handle, Duplicate, right-click menu |
| Grid and naming | Read only | Inspector: grid, pattern sentence, live matches | Start number, step, letters, direction | — |
| Draft from bin list | Not available | Button on the empty canvas; Unplaced tray | Preview counts | — |
| Delete | Not available | Delete (bins go to Unplaced, with a preview) | — | Delete key, right-click menu |
| Undo | *Undo* on the saved banner | Named Undo in the Layout menu | — | Ctrl+Z |
| Turn snapping off | — | Snap toggle | — | Hold Alt |

### What the evidence says not to build

The evidence also points to things to leave out:

- **A global "Advanced mode" switch.** Logic just removed one.
- **Menus that reorder themselves by usage.** They lost on both speed and preference.
- **Phone scanning of the building.** A warehouse is several times beyond RoomPlan's documented limits for span, height and area.
- **A step-by-step tutorial wizard.** This is the opposite of Apple's guidance on contextual tips.
- **Measurements anywhere in the interface.** They invite a precision the model deliberately does not have.
- **"Turn left" style directions.** Without a known direction of approach, they risk mirror-image errors.
- **Editing in 3D.** Novices lose their bearings there.
- **Removing an expert tool once it has shipped.** Final Cut Pro X and iWork both show what that costs.

## Conclusion

The most useful thing this research changes is the answer to who the layout editor is for. The instinct is to build a simplified layout editor for floor staff. The evidence says to spare them from editing geometry at all. Spork's model already separates exact relationships from approximate positions, and that lets the work divide cleanly. The fact floor staff are best placed to supply, which bin is in which cell, arrives through a scan at the shelf, something they already do every shift. The geometry, which punishes novices, stays at a desk where snapping, constraints and undo absorb mistakes. This is Tesler's conservation of complexity in practice: the software carries the complexity, not the worker, the same way Final Cut's magnetic timeline keeps clips in sync so editors don't have to.

What remains unknown is whether this works for the people it is meant for. No source tested layout software with warehouse staff who have low digital skills. The strongest picking evidence comes from eight students in a lab, and Apple publishes no usability data at all. Apple's 1992 guidelines advise designers to "visit actual work places and study how people do their jobs" ([1992 HIG](https://vintageapple.org/inside_r/pdf/Human_Interface_Guidelines_1992.pdf)). GOV.UK says research with people who have little or no digital skill must be done offline and one to one ([GOV.UK Service Manual](https://www.gov.uk/service-manual/helping-people-to-use-your-service/assisted-digital-support-introduction)). The first build of the place page should therefore be tested on the floor against the bare bin code, timing how long workers take to reach the right cell, before the priority 2 and 3 work is polished. The paradigm to test is **one place model, three depths, no modes**, with exact facts coming from the floor and geometry drawn at the desk.
