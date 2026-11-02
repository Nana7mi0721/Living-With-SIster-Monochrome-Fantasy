import json, glob, os

ROOT="work/wolftl/dump/common"
MVT="work/translation/mv_text.json"
OUT="work/translation/final"
BS=chr(92); TAB=chr(9); NL=chr(10)

def strip_tags(s):
    if not isinstance(s,str): s=str(s)
    out=[]; i=0; n=len(s)
    while i<n:
        ch=s[i]
        if ch==BS:
            i+=1
            if i<n:
                c=s[i]
                if c=="N" and i+1<n and s[i+1]=="<":
                    j=s.find(">", i+2); i=(j+1 if j!=-1 else n)
                elif c in "cC" and i+1<n and s[i+1]=="[":
                    j=s.find("]", i+2); i=(j+1 if j!=-1 else n)
                elif c in "eE":
                    i+=1
                else:
                    i+=1
            continue
        out.append(ch); i+=1
    return "".join(out)

def norm(s):
    s=strip_tags(s)
    return "".join(ch for ch in s if not ch.isspace())

def load_wolf():
    res={}
    for f in glob.glob(ROOT+"/*.json"):
        base=os.path.basename(f); num=""
        for ch in base:
            if ch.isdigit(): num+=ch
            else: break
        if not num: continue
        cid=int(num); title=base[len(num)+1:-5]
        d=json.load(open(f,encoding="utf-8")); rows=[]
        def walk(x):
            if isinstance(x,dict):
                if x.get("code")==101:
                    sa=x.get("stringArgs") or []
                    rows.append((x.get("index",0),"".join(str(t) for t in sa)))
                for v in x.values(): walk(v)
            elif isinstance(x,list):
                for v in x: walk(v)
        walk(d); rows.sort(key=lambda r:r[0])
        res[cid]=(title,rows)
    return res

def load_mv():
    d=json.load(open(MVT,encoding="utf-8")); res={}
    for e in d["records"]:
        if e.get("code")!=401 or e.get("file")!="CommonEvents.json": continue
        res.setdefault(e.get("event"),[]).append((e.get("idx") or 0,e.get("text") or ""))
    for k in list(res): res[k]=[t for _,t in sorted(res[k])]
    return res

def feats(txts):
    return [(set(norm(t)), len(norm(t))) for t in txts]

def sim_pre(a,b):
    sa,la=a; sb,lb=b
    if la==0 and lb==0: return 0.0
    inter=len(sa & sb)
    dice=(2.0*inter/(len(sa)+len(sb))) if (sa or sb) else 0.0
    mx=max(la,lb); lr=(min(la,lb)/mx) if mx>0 else 0.0
    return 0.5*dice+0.5*lr

def align_dp(A,B):
    n=len(A); m=len(B)
    if n==0 or m==0: return []
    gap=-0.25; NEG=-1e9
    dp=[[NEG]*(m+1) for _ in range(n+1)]; bt=[[0]*(m+1) for _ in range(n+1)]
    dp[0][0]=0.0
    for i in range(1,n+1): dp[i][0]=i*gap; bt[i][0]=1
    for j in range(1,m+1): dp[0][j]=j*gap; bt[0][j]=2
    for i in range(1,n+1):
        di=dp[i]; dim=dp[i-1]; bti=bt[i]; ai=A[i-1]
        for j in range(1,m+1):
            s=sim_pre(ai,B[j-1]); v1=dim[j-1]+s; v2=dim[j]+gap; v3=di[j-1]+gap
            if v1>=v2 and v1>=v3: di[j]=v1; bti[j]=0
            elif v2>=v3: di[j]=v2; bti[j]=1
            else: di[j]=v3; bti[j]=2
    i=n; j=m; pairs=[]
    while i>0 or j>0:
        d=bt[i][j]
        if i>0 and j>0 and d==0: pairs.append((i-1,j-1)); i-=1; j-=1
        elif i>0 and d==1: i-=1
        else: j-=1
    pairs.reverse(); return pairs

def load_terms():
    out=[]; p="work/translation/glossary_canonical.tsv"
    for ln in open(p,encoding="utf-8").read().split(NL):
        if not ln or ln.startswith("term_id"): continue
        c=ln.split(TAB)
        if len(c)<6: continue
        bim,can,onimg=c[3],c[4],c[5]
        if onimg.strip().lower()=="true" and bim and can and can!=bim: out.append((bim,can))
    return out

def rewrite(txt,terms):
    o=txt
    for s,d in terms:
        if s in o: o=o.replace(s,d)
    return o

def main():
    W=load_wolf(); M=load_mv(); terms=load_terms()
    os.makedirs(OUT,exist_ok=True)
    rows=[]; ev=[]; stats={"events":0,"ev_count":0,"ev_dp":0,"ev_mismatch":0,"ev_official":0,"wolf_msgs":0,"bimeng":0,"official":0,"dp_pairs":0,"dp_conf":0,"term_rows":0}
    lowconf=[]
    for cid in sorted(W):
        title,wrows=W[cid]
        if not wrows: continue
        stats["events"]+=1
        wtxt=[t for _,t in wrows]; mtext=M.get(cid); method="official"; pairs=[]
        if mtext:
            if len(wtxt)==len(mtext): pairs=[(i,i) for i in range(len(wtxt))]; method="count"; stats["ev_count"]+=1
            else:
                A=feats(wtxt); B=feats(mtext); pairs=align_dp(A,B)
                conf=sum(1 for i,j in pairs if sim_pre(A[i],B[j])>=0.35)
                ratio=conf/len(wtxt) if wtxt else 0.0
                if ratio<0.60:
                    method="mismatch"; pairs=[]; stats["ev_mismatch"]+=1
                else:
                    method="dp"; stats["ev_dp"]+=1; stats["dp_pairs"]+=len(pairs); stats["dp_conf"]+=conf
        else:
            stats["ev_official"]+=1
        mp={i:j for i,j in pairs}
        A=feats(wtxt); B=feats(mtext) if mtext else None
        b=o=0; ssum=0.0
        for i,(idx,official) in enumerate(wrows):
            stats["wolf_msgs"]+=1
            bim=""; src="official"; s=0.0; mtd="none"
            if i in mp and mtext:
                j=mp[i]; bim=mtext[j]; s=sim_pre(A[i],B[j]); mtd=method
                if method=="count" or s>=0.35: src="bimeng"
            final=rewrite(bim,terms) if src=="bimeng" else official
            if src=="bimeng" and final!=bim: stats["term_rows"]+=1
            ftext=strip_tags(final)
            if src=="bimeng": stats["bimeng"]+=1; b+=1
            else: stats["official"]+=1; o+=1
            ssum+=s
            rows.append({"event":cid,"index":idx,"official_zh":official,"bimeng_zh":bim,"final_zh":final,"final_text":ftext,"source":src,"sim":round(s,3),"method":mtd})
            if src=="official" and mtd=="dp" and s>0.0 and s<0.55:
                lowconf.append((cid,idx,official,bim,round(s,3)))
        avg=(ssum/len(wrows)) if wrows else 0.0
        reason="ok"
        if not mtext:
            reason=("dlc_absent_in_mv" if "DLC" in title else ("mv_empty_title" if title in ("",".json") else "mv_no_text"))
        elif method=="mismatch":
            reason="mv_id_collision"
        ev.append({"event":cid,"title":title,"wolf":len(wrows),"bimeng":b,"official":o,"method":method,"avg_sim":round(avg,3),"reason":reason})
    # writers
    with open(OUT+"/wolf_text_AB.jsonl","w",encoding="utf-8") as f:
        for r in rows: f.write(json.dumps(r,ensure_ascii=False)+NL)
    with open(OUT+"/final_text.jsonl","w",encoding="utf-8") as f:
        for r in rows: f.write(json.dumps({"event":r["event"],"index":r["index"],"text":r["final_text"],"source":r["source"]},ensure_ascii=False)+NL)
    with open(OUT+"/summary_by_event.tsv","w",encoding="utf-8") as f:
        f.write(TAB.join(["event","title","wolf_msgs","bimeng","official","method","avg_sim","reason"])+NL)
        for e in ev: f.write(TAB.join(str(e[k]) for k in ["event","title","wolf","bimeng","official","method","avg_sim","reason"])+NL)
    with open(OUT+"/review_lowconf.tsv","w",encoding="utf-8") as f:
        f.write(TAB.join(["event","index","official_zh","bimeng_zh","sim"])+NL)
        for t in lowconf: f.write(TAB.join(str(x).replace(TAB," ").replace(NL," ") for x in t)+NL)
    genuine={"count","dp"}
    tg=sum(e["wolf"] for e in ev if e["method"] in genuine)
    bg=sum(e["bimeng"] for e in ev if e["method"] in genuine)
    tu=sum(e["wolf"] for e in ev if e["method"] not in genuine)
    # report
    L=[]
    L.append("# 策略 A 翻译流水线报告（首版）")
    L.append("")
    L.append("## 1. 方法与数据源")
    L.append("- 官方中文 = WOLF 数据 CE 的 101 Message（work/wolftl/dump/common/*.json）")
    L.append("- 彼梦精翻 = MV 移植 CommonEvents.json 的 401（work/translation/mv_text.json）")
    L.append("- 事件级：同 id 对齐（MV 是 WOLF 的重移植，CE id 与事件顺序基本保留；MV 事件名 = WOLF 标题的中译，作健全性旁证）。")
    L.append("- 消息级：同事件消息数相等 -> 按顺序 1:1（count，最稳）；不等 -> Needleman-Wunsch 单调对齐，相似度 = 0.5*字符集 Dice + 0.5*长度比，gap = -0.25；单对 >=0.35 才采纳彼梦，否则回落官方。")
    L.append("- 归一化：glossary_canonical.tsv 中 on_image=true 的术语把彼梦词替换成图片/技能栏用官方标准（本轮命中 %d 行）。" % stats["term_rows"])
    L.append("- emit：final_text 去掉 E 前缀 / N<名> / c[] 等控制标签；final_zh 保留原始标签供对照。")
    L.append("")
    L.append("## 2. 覆盖统计")
    L.append("- WOLF 有台词事件 %d 个，台词行 %d 行。" % (stats["events"],stats["wolf_msgs"]))
    L.append("- 事件分类：count(等长1:1)=%d，dp(不等长对齐)=%d，mismatch(同id但事件不同，整事件回落官方)=%d，official(无彼梦同id事件)=%d。" % (stats["ev_count"],stats["ev_dp"],stats["ev_mismatch"],stats["ev_official"]))
    L.append("- 行来源：彼梦 %d 行（%.1f%%），官方回落 %d 行（%.1f%%）。" % (stats["bimeng"],100.0*stats["bimeng"]/stats["wolf_msgs"],stats["official"],100.0*stats["official"]/stats["wolf_msgs"]))
    L.append("- 关键：在彼梦【确有对应事件】的行里（count+dp 事件共 %d 行），采纳彼梦 %.1f%%（%d 行）；其余 %d 行属于彼梦移植没有的 DLC/相册事件，只能留在官方。" % (tg,100.0*bg/max(1,tg),bg,tu))
    L.append("- DP 对齐对数 %d，其中相似度>=0.35 的 %d（%.1f%%）。" % (stats["dp_pairs"],stats["dp_conf"],100.0*stats["dp_conf"]/max(1,stats["dp_pairs"])))
    L.append("- 低置信（0.0<sim<0.55）待人工复核：%d 行，见 review_lowconf.tsv。" % len(lowconf))
    L.append("")
    L.append("## 3. CE280 黄金样本（count 1:1）")
    L.append("event/index | source | final_text")
    for r in rows:
        if r["event"]==280:
            L.append("%d/%d | %s | %s" % (r["event"],r["index"],r["source"],r["final_text"]))
    L.append("")
    L.append("## 4. 回落官方的主因（彼梦移植未含/编号错位）")
    top=sorted(ev,key=lambda e:-e["official"])[:15]
    L.append("event | title | wolf | official | reason")
    for e in top:
        if e["official"]>0:
            L.append("%d | %s | %d | %d | %s" % (e["event"],e["title"],e["wolf"],e["official"],e["reason"]))
    L.append("")
    L.append("## 5. 质量与风险")
    L.append("- count 事件按顺序映射，平均相似度如下（<0.55 需抽查）：")
    for e in sorted(ev,key=lambda x:x["event"]):
        if e["method"]=="count":
            L.append("  - CE%d n=%d avg_sim=%s%s" % (e["event"],e["wolf"],e["avg_sim"],"  <-偏低抽查" if e["avg_sim"]<0.55 else ""))
    L.append("- DP 事件为单调对齐，可能出现个别误配；已用 0.35 阈值把低相似对降级为官方，并导出 review_lowconf.tsv。")
    L.append("- DLC 剧情/相册处理事件（标题含 DLC、或 CE522-527 相册显示、CE452 等）在彼梦移植中没有对应中文，只能保留官方；这不是对齐缺陷。")
    L.append("- 控制标签：final_text 已清洗；若要写回 WOLF，需要另做标签/说话人重建（N 名字 -> WOLF 名字字段/演讲者）。")
    L.append("")
    L.append("## 6. 产物")
    L.append("- final/wolf_text_AB.jsonl：逐行 official/bimeng/final/source/sim/method")
    L.append("- final/final_text.jsonl：逐行最终文本（已清洗标签）+ source")
    L.append("- final/summary_by_event.tsv：按事件统计")
    L.append("- final/review_lowconf.tsv：待人工复核的低置信行")
    open(OUT+"/REPORT_A_PIPELINE.md","w",encoding="utf-8").write(NL.join(L)+NL)
    print("STATS",json.dumps(stats,ensure_ascii=False))
    print("wrote",OUT+"/REPORT_A_PIPELINE.md","rows",len(rows),"lowconf",len(lowconf))

if __name__=="__main__":
    main()
