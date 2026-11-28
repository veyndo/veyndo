"""Brand theme draft → private preview → reviewed release: the live shop keeps its design until the selected release; invalid themes never save."""
import copy
import json
import pathlib

FIXTURE = pathlib.Path(__file__).resolve().parents[2] / 'extensions/apps/site-builder/tests/brand-theme-fixture.json'


def verify(call, account, headers):
    brand = json.loads(FIXTURE.read_text())['vivid']
    owner = account('brand-theme')
    live = headers(owner)
    public = {'x-tenant': owner['workspace']}
    pages = '/api/apps/site_builder/actions/'
    call('/api/apps', {'builtIn': 'site_builder'}, live)
    home = {'channel': '*', 'slug': 'home', 'title': {'en-GB': 'Home'}, 'kind': 'landing', 'status': 'published',
            'starts_at': None, 'content': {'blocks': [{'id': 'hero', 'kind': 'hero', 'text': {'en-GB': 'Welcome'}}]},
            'theme': {'accent': '#0877e8'}, 'seo': None, 'author': None, 'category': None}
    call(pages + 'save_page', {'id': 'home', 'revision': 0, 'fields': home}, live)
    published = lambda: next(p for p in call('/store-api/apps/site_builder/actions/pages', {}, public)['elements'] if p['id'] == 'home')
    assert 'brand' not in published()['theme']

    # Draft: the generated theme goes into a private sandbox only.
    stage = call('/api/environments', {'name': 'Brand theme'}, live)['id']
    private = headers(owner, stage)
    draft = next(p for p in call(pages + 'editor_pages', {}, private)['elements'] if p['id'] == 'home')
    themed = copy.deepcopy(home)
    themed['theme'] = {'accent': brand['colors']['accent'], 'background': brand['colors']['surface'],
                       'foreground': brand['colors']['text'], 'font': 'sans', 'radius': 24, 'spacing': 40,
                       'motion': 'expressive', 'brand': brand}
    # The server enforces the token contract: contrast, vetted fonts, home-only brand and known intensities.
    for broken in ({'muted': brand['colors']['surface']}, {'accentText': brand['colors']['accent']}):
        bad = copy.deepcopy(themed)
        bad['theme']['brand']['colors'].update(broken)
        call(pages + 'save_page', {'id': 'home', 'revision': draft['revision'], 'fields': bad}, private, 400)
    bad = copy.deepcopy(themed)
    bad['theme']['brand']['fonts']['heading'] = 'papyrus'
    call(pages + 'save_page', {'id': 'home', 'revision': draft['revision'], 'fields': bad}, private, 400)
    bad = copy.deepcopy(themed)
    bad['theme']['motion'] = 'wild'
    call(pages + 'save_page', {'id': 'home', 'revision': draft['revision'], 'fields': bad}, private, 400)
    about = copy.deepcopy(themed)
    about.update(slug='about', title={'en-GB': 'About'})
    call(pages + 'save_page', {'id': 'about', 'revision': 0, 'fields': about}, private, 400)
    call(pages + 'save_page', {'id': 'home', 'revision': draft['revision'], 'fields': themed}, private)

    # Preview: the sandbox shows the theme, the live shop and its public read do not.
    preview = next(p for p in call(pages + 'editor_pages', {}, private)['elements'] if p['id'] == 'home')
    assert preview['theme']['brand'] == brand and preview['theme']['motion'] == 'expressive'
    assert 'brand' not in next(p for p in call(pages + 'editor_pages', {}, live)['elements'] if p['id'] == 'home')['theme']
    assert 'brand' not in published()['theme']

    # Publish: only the selected, reviewed release changes the live shop.
    change = next(c for c in call('/api/environments/' + stage + '/diff', h=live)['changes'] if c['key'] == 'appdata:site_builder:pages:home')
    release = '/api/environments/' + stage + '/release'
    call(release, {'approve': False, 'selections': [{'key': change['key'], 'digest': change['digest']}]}, live, 400)
    assert 'brand' not in published()['theme']
    call(release, {'approve': True, 'selections': [{'key': change['key'], 'digest': change['digest']}]}, live)
    live_theme = published()['theme']
    assert live_theme['brand'] == brand and live_theme['motion'] == 'expressive'
    versions = call('/api/apps/site_builder/entities/pages/home/history', h=live)['versions']
    assert any('brand' not in v['fields']['theme'] for v in versions), 'the previous design stays restorable'
    print('PASS brand theme draft → private preview → reviewed release; contrast/font/scope/motion refusals; live unchanged until publication')
