import React from 'react';
import AppState from './appstate.jsx'

import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';

import Chart from "react-google-charts";

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';
import Paper from '@material-ui/core/Paper';

import TextField from '@material-ui/core/TextField';
import InputLabel from '@material-ui/core/InputLabel';
import Input from '@material-ui/core/Input';
import ListItemText from '@material-ui/core/ListItemText';
import MenuItem from '@material-ui/core/MenuItem';
import Select from '@material-ui/core/Select';


import PersonForm from './tour-person-edit.jsx'
import SpendingForm from './tour-spending-edit.jsx'
import SpendingsDetail from './person-show-spendings.jsx'

import Checkbox from '@material-ui/core/Checkbox';

import createBrowserHistory from 'history/createBrowserHistory';

import Tabs from '@material-ui/core/Tabs';
import Tab from '@material-ui/core/Tab';
import Button from '@material-ui/core/Button';
import Box from '@material-ui/core/Box';

import { sizing } from '@material-ui/system';

import TourInfo from './tour-info.jsx'

const history = createBrowserHistory();

const ITEM_HEIGHT = 48;
const ITEM_PADDING_TOP = 8;
const MenuProps = {
    PaperProps: {
        style: {
            maxHeight: ITEM_HEIGHT * 4.5 + ITEM_PADDING_TOP,
            width: 250,
        },
    },
};


export default class TourUI extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
        }
    }

    componentDidMount() {
        AppState.checkWhoAmI(this);
    }

    render() {
        if (!this.state.isAuthLoaded) {
            return (<div>Checking Who you are...</div>)
        } else {
            if (!this.state.authData.isMaster
                && this.state.authData.tourIds.indexOf(this.props.tourid) == -1) { // no such tour for credentials
                //return (<TourRequestAccessCode app={this} />)
                //alert("authdata: " + JSON.stringify(this.state.authData));
                if (this.state.authData.type == "AccessCode") { // just wrong access code, or tour is in another code
                    return <Redirect to="/" />
                } else {
                    return (<TourRequestAccessCode app={this} />)
                }

            } else {
                return (<TourTable tourid={this.props.tourid} />)
            }

        }

    }
}
/*const useStyles = makeStyles(theme => ({
    root: {
        width: '100%',
        marginTop: theme.spacing(3),
        overflowX: 'auto',
    },
    table: {
        minWidth: 650,
    },
}));
const classes = useStyles();*/

class TourTable extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isTourLoaded: false,
            tour: null,
            showSuggested: false,
            dialogPOpen: false,
            currentPage: 'persons',
            updateTime: new Date(),
            filterByCat: []
        }
    }

    componentDidMount() {
        document.title = "Toucalc: Tour loading"
        AppState.loadTour(this, this.props.tourid);
        this.interval = setInterval(
            () => {
                AppState.loadTour(this, this.props.tourid)
                    .then(this.setState({ updateTime: new Date() }))
            }
            , (60000 * 3)) // once a 3 min
    }

    componentWillUnmount() {
        clearInterval(this.interval);
    }

    getSpendingsSummary(sp) {
        var ss = {};
        ss.total = 0;
        ss.cats = {};
        sp.filter(s => s.type != null && s.type != "").forEach(s => {
            ss.total += s.amountInCents
            if (ss.cats[s.type] == null) ss.cats[s.type] = 0;
            ss.cats[s.type] += s.amountInCents;
        })
        ss.sortedSummary = Object.keys(ss.cats).sort(
                (k1, k2) => {
                    return -(ss.cats[k1] > ss.cats[k2] ? 1 : (ss.cats[k1] < ss.cats[k2] ? -1 : 0))
                }
        ).map((key) => { return { 'cat': key, 'amount': ss.cats[key] } });
        
        return ss;
    }

    lastSpendingType(tour) {
        let sppp = tour.spendings.filter(s => !s.planned && s.type);
        return sppp.length > 0
            ? sppp[sppp.length - 1].type
            : "Common"
    }
    lastSpenderGUID(tour) {
        let sppp = tour.spendings.filter(s => !s.planned && s.type);
        return sppp.length > 0
            ? sppp[sppp.length - 1].fromGuid
            : null
    }
    isSuggestedSet = false;
    render() {
        if (!this.state.isTourLoaded) {
            return <div>Tour {this.props.tourid} loading...</div>
        } else {

            if (this.state.tour == null) { // loaded, but null (no such tour)
                return <Redirect to="/"/>
            }
            if (!this.isSuggestedSet) {
                this.state.showSuggested = this.state.tour.isFinalizing;
                this.isSuggestedSet = true;
                //alert("ssss!!!!");
            }
            document.title = "Tourcalc: " + this.state.tour.name;
            this.spSummary = this.getSpendingsSummary(this.state.tour.spendings.filter(s => !s.planned))

            return (
                <Router>
                    <div style={{ maxWidth: 1080, border: "0px double black"}}>

                        {/*--- Tabs ---*/}
                        <Tabs
                            value={(history.location.pathname === '/' || !history.location.pathname.startsWith('/tour/' + this.props.tourid)) ? '/tour/' + this.props.tourid + '/persons' : history.location.pathname}
                            indicatorColor="primary"
                            textColor="primary"
                            onChange={(event, v) => { history.push(v); this.setState({ currentPage: v }) }}
                            aria-label="Tourcalc nav">

                            <Tab label="Persons" value={'/tour/' + this.props.tourid + '/persons'}
                                component={Link} to={'/tour/' + this.props.tourid + '/persons'}
                            />
                            <Tab label="Spendings" value={'/tour/' + this.props.tourid + '/spendings'}
                                component={Link} to={'/tour/' + this.props.tourid + '/spendings'}
                            />
                        </Tabs>
                        {/*--- /Tabs ---*/}


                        &nbsp;<TourInfo tour={this.state.tour} app={this} updateTime={this.state.updateTime} />

                        <main>
                            <Switch>
                                <Route exact path={'/tour/' + this.props.tourid}
                                    render={(props) => <Redirect to={'/tour/' + this.props.tourid + '/persons'} />}
                                />
                                {/*--- Spendings ---*/}

                                <Route path={'/tour/' + this.props.tourid + '/spendings'}
                                    render={(props) => (

                                        <Table stickyHeader>
                                            <TableHead>
                                                <TableRow>
                                                    <TableCell colSpan={5} align="left">
                                                        <Select
                                                            multiple
                                                            variant='filled'
                                                            style={{
                                                                minWidth: 200,
                                                                maxWidth: 900
                                                            }
                                                            }
                                                            value={this.state.filterByCat}
                                                            onChange={(e) => {
                                                                this.setState({ filterByCat: e.target.value.includes("_RESET_ALL") ? [] : e.target.value });
                                                            }}
                                                            input={<TextField id="select-multiple-checkbox" label="Category Filter"/>}

                                                            renderValue={selected => selected.length > 0 ? selected.join(', ') : 'Choose...'}
                                                            MenuProps={MenuProps}
                                                        >
                                                            <MenuItem key="_RESET_ALL" value="_RESET_ALL">
                                                                <ListItemText primary="Reset" />
                                                            </MenuItem>
                                                            {this.spSummary
                                                                .sortedSummary.map(s => (
                                                                    <MenuItem key={s.cat} value={s.cat}>
                                                                        <Checkbox checked={this.state.filterByCat.includes(s.cat)} />
                                                                        <ListItemText primary={s.cat} />
                                                                    </MenuItem>
                                                                ))}
                                                        </Select>
                                                    </TableCell>
                                                </TableRow>
                                                <TableRow>
                                                    <TableCell>Spending Description
                                                   { this.state.tour.isVersion ? <span/> :
                                                            <SpendingForm
                                                                tour={this.state.tour}
                                                                buttonText="Add"
                                                                actionButtonText="Save Spending"
                                                                open={false}
                                                                mode="add"
                                                                app={this}
                                                                spending={{
                                                                    description: "",
                                                                    amountInCents: 0,
                                                                    fromGuid: this.lastSpenderGUID(this.state.tour),
                                                                    toGuid: [],
                                                                    toAll: true,
                                                                    guid: "",
                                                                    type: this.lastSpendingType(this.state.tour)
                                                                }}
                                                            ><Button color='primary' variant='outlined'>Add</Button></SpendingForm>
                                                        }
                                                        &nbsp;
                                                        <Button color='secondary' variant='outlined'
                                                            onClick={() => { this.setState({ showSuggested: !this.state.showSuggested }) }}>
                                                            S: {this.state.showSuggested ? 'hide' : 'show'}
                                                        </Button>
                                                        Finalizing: <input
                                                            type="checkbox"
                                                            defaultChecked={this.state.tour.isFinalizing}
                                                            onChange={(e) => {
                                                                //this.state.tour.isFinalizing = !this.state.tour.isFinalizing;
                                                                AppState.changeTourFinalizing(this, this.state.tour.id, !this.state.tour.isFinalizing, "")
                                                                    .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                    .then(() => { this.isSuggestedSet = false; });
                                                            }} />

                                                        
                                                    </TableCell>
                                                    <TableCell align="right">From</TableCell>
                                                    <TableCell align="right">Amount</TableCell>
                                                    <TableCell align="right">To all</TableCell>
                                                    <TableCell align="right">Recipients
                                                        
                                                        </TableCell>
                                                </TableRow>

                                            </TableHead>
                                            <TableBody>
                                                {this.state.tour.spendings

                                                    .sort((s1, s2) => {
                                                        if (s1.planned && !s2.planned) return -1;
                                                        if (!s1.planned && s2.planned) return 1;
                                                        if (s1.planned && s2.planned) {
                                                            if (s1.description.startsWith('Family') && !s2.description.startsWith('Family')) return -1;
                                                            if (!s1.description.startsWith('Family') && s2.description.startsWith('Family')) return 1;
                                                            if (s1.amountInCents > s2.amountInCents) return -1;
                                                            if (s1.amountInCents < s2.amountInCents) return 1;
                                                            return 0;
                                                        }
                                                        if (s1.dateCreated > s2.dateCreated) return 1;
                                                        if (s1.dateCreated < s2.dateCreated) return -1;
                                                        return 0;
                                                    })

                                                    .filter((sp) =>
                                                        (this.state.showSuggested || !sp.planned)
                                                        && (sp.planned || (this.state.filterByCat.length == 0 || this.state.filterByCat.includes(sp.type)))
                                                    )
                                                    .map((p, idx) => (
                                                        <TableRow key={p.guid} hover

                                                            style={

                                                                p.planned
                                                                    ?
                                                                    /*planned*/(p.description.startsWith('Family') ? { background: "cyan" } : { background: "yellow" })
                                                                    :
                                                                    /*not planned, real spending*/(p.toGuid.length == 1 ? { background: "#FFFCF3" } : {})}

                                                            selected={idx % 2 == 0 ? true : false}
                                                        >
                                                            <TableCell component="th" scope="row">

                                                                <span style={{ cursor: 'pointer', borderStyle: p.planned ? 'none' : 'ridge', fontSize: 'xx-small' }}
                                                                    variant='outlined' color='secondary' onClick={() => {
                                                                        if (window.confirm('Sure to delete spending ' + p.description + '?')) {
                                                                            AppState.deleteSpending(this, this.props.tourid, p.guid)
                                                                                .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                        }
                                                                    }}>{p.planned ? '' : 'X'}</span>

                                                                &nbsp;
                                                                {(idx + 1) + '.'}
                                                                {(!p.planned) ?
                                                                    <SpendingForm
                                                                        tour={this.state.tour}
                                                                        buttonText={p.description}
                                                                        actionButtonText="Save Spending"
                                                                        open={false}
                                                                        mode="edit"
                                                                        app={this}
                                                                        spending={p}
                                                                    >
                                                                        <span style={{ cursor: 'pointer', textDecoration: 'underline' }}>
                                                                            {p.description}</span><br/>
                                                                        <span style={{ fontSize: 'xx-small' }}>[

                                                                            {
                                                                                new Intl.DateTimeFormat("ru-RU", {
                                                                                    year: 'numeric', month: 'numeric', day: 'numeric',
                                                                                    hour: 'numeric', minute: 'numeric', second: 'numeric',
                                                                                    hour12: false
                                                                                }).format(Date.parse(p.dateCreated))
                                                                            }

                                                                            ]  <b>{p.type}</b> </span>
                                                                    </SpendingForm>
                                                                    : <span>{p.description}&nbsp;

                                                                        {this.state.tour.isVersion ? <span /> :
                                                                            <SpendingForm
                                                                                tour={this.state.tour}
                                                                                buttonText="Add"
                                                                                actionButtonText="Save Spending"
                                                                                open={false}
                                                                                mode="add"
                                                                                app={this}
                                                                                spending={(() => {
                                                                                    var pp = JSON.parse(JSON.stringify(p))
                                                                                    pp.type = ""; // so it'll not show in summary
                                                                                    return pp;
                                                                                })()}
                                                                            ><Button color='primary' variant='outlined'>Add</Button></SpendingForm>
                                                                        }

                                                                    </span>
                                                                }
                                                            </TableCell>
                                                            <TableCell align="right">{
                                                                this.state.tour.persons.filter((pp) => pp.guid === p.fromGuid).map(ppp => ppp.name)
                                                            }</TableCell>
                                                            <TableCell align="right">
                                                                {p.amountInCents}
                                                            </TableCell>
                                                            <TableCell align="right"><Checkbox checked={p.toAll} disabled /></TableCell>
                                                            <TableCell align="right" style={{ fontSize: "xx-small" }}>{p.toAll ? (<i style={{ backgroundColor: 'green', color: 'yellow' }}>ALL</i>) : p.toGuid.map(

                                                                (id) => (this.state.tour.persons.find((pp) => pp.guid === id) == null)
                                                                    ? '$$-' + id + '-$$'
                                                                    : this.state.tour.persons.find((pp) => pp.guid === id).name

                                                            ).join(', ')}</TableCell>
                                                        </TableRow>
                                                    ))}
                                                <TableRow>
                                                    <TableCell colSpan={2}>
                                                        <b>Summary:</b> <br />
                                                        <p>TOTAL: <b>{this.spSummary.total}</b></p>

                                                        <Chart
                                                            width={'100%'}
                                                            height={'100%'}
                                                            chartType="Table"
                                                            loader={<div>Loading Chart</div>}
                                                            options={{
                                                                showRowNumber: false,
                                                            }}
                                                            data={
                                                                [
                                                                    [{ type: 'string', label: 'Категория' },
                                                                    { type: 'number', label: 'Сумма' },
                                                                    { type: 'string', label: 'Процент' },
                                                                    ]
                                                                ]
                                                                    .concat(
                                                                        this.spSummary
                                                                            .sortedSummary.map((cat, idx) => [cat.cat, { v: cat.amount }, (cat.amount * 100 / this.spSummary.total).toFixed(2) + "%"])

                                                                    )

                                                            }
                                                            rootProps={{ 'data-testid': '1' }}
                                                        />
                                                        
                                                    </TableCell>
                                                    <TableCell colSpan={3}>
                                                        <Chart
                                                            width={'500px'}
                                                            height={'300px'}
                                                            chartType="PieChart"
                                                            loader={<div>Loading Chart</div>}
                                                            options={{
                                                                chartArea: { width: '100%', height: '80%' },
                                                            }}
                                                            data={
                                                                [['Category', 'Amount']]
                                                                    .concat(
                                                                    this.spSummary
                                                                        .sortedSummary.map((cat, idx) => {
                                                                            return [cat.cat, cat.amount]
                                                                        })
                                                                    )

                                                            }
                                                            rootProps={{ 'data-testid': '1' }}
                                                        />
                                                    </TableCell>
                                                </TableRow>
                                            </TableBody>
                                        </Table>


                                    )} />

                                {/*--- /Spendings ---*/}


                                {/*--- Persons ---*/}

                                <Route path={'/tour/' + this.props.tourid + '/persons'}
                                    render={(props) => (
                                        <Table stickyHeader>
                                            <TableHead>
                                                <TableRow>
                                                    <TableCell>Person Name
                                                        {this.state.tour.isVersion ? <span /> :
                                                            <PersonForm mode="add"
                                                                tourid={this.props.tourid}
                                                                tour={this.state.tour}
                                                                open={false}
                                                                app={this}
                                                                buttonText="Add" actionButtonText="Add Person" ><Button color='primary' variant='outlined'>Add</Button>
                                                            </PersonForm>
                                                        }
                                                    </TableCell>
                                                    <TableCell align="right">#</TableCell>
                                                    <TableCell align="right">Weight %</TableCell>
                                                    <TableCell align="right">Spent</TableCell>
                                                    <TableCell align="right">Received</TableCell>
                                                    <TableCell align="right">Debt</TableCell>
                                                </TableRow>
                                            </TableHead>
                                            <TableBody>
                                                {this.state.tour.persons

                                                    .sort((p1, p2) => {
                                                        var n1 = (this.state.tour.persons.find((pp) => pp.guid == p1.parentId) == null
                                                            ? ''
                                                            : this.state.tour.persons.find((pp) => pp.guid == p1.parentId).name)
                                                                + p1.name;
                                                        var n2 = (this.state.tour.persons.find((pp) => pp.guid == p2.parentId) == null
                                                            ? ''
                                                            : this.state.tour.persons.find((pp) => pp.guid == p2.parentId).name)
                                                                + p2.name;
                                                        if (n1 > n2) return 1;
                                                        if (n1 < n2) return -1;
                                                        return 0;
                                                    })

                                                    .map((p, idx) => (
                                                    <TableRow key={p.guid} hover={true} selected={idx%2==0 ? true : false}>
                                                        <TableCell component="th" scope="row">
                                                            <span style={{ cursor: 'pointer', borderStyle: 'ridge', fontSize: 'xx-small' }} onClick={() => {
                                                                if (window.confirm('Sure to delete ' + p.name + '?')) {
                                                                    AppState.deletePerson(this, this.props.tourid, p.guid)
                                                                        .then(() => { AppState.loadTour(this, this.props.tourid); })
                                                                }
                                                            }}>X</span>
                                                            &nbsp;
                                                            {(idx + 1) + '.'}
                                                            <PersonForm mode="edit"
                                                                tourid={this.props.tourid}
                                                                tour={this.state.tour}
                                                                open={false}
                                                                app={this}
                                                                buttonText={p.name} actionButtonText="Save Person"
                                                                person={p}
                                                                guid={p.guid}
                                                            ><span style={{ cursor: 'pointer', textDecoration: 'underline' }}>{p.name}

                                                                </span></PersonForm>
                                                            <span style={{ fontSize: 'xx-small' }}>
                                                                {this.state.tour.persons.find((pp) => pp.guid == p.parentId) == null
                                                                    ? ''
                                                                    : ' > ' + this.state.tour.persons.find((pp) => pp.guid == p.parentId).name}
                                                            </span>
                                                        </TableCell>
                                                        <TableCell align="right">
                                                            {this.state.tour.isVersion ? <span /> :
                                                                <SpendingForm
                                                                    tour={this.state.tour}
                                                                    buttonText="Add"
                                                                    actionButtonText="Save Spending"
                                                                    open={false}
                                                                    mode="add"
                                                                    app={this}
                                                                    spending={{
                                                                        description: "",
                                                                        amountInCents: 0,
                                                                        fromGuid: p.guid,
                                                                        toGuid: [],
                                                                        toAll: true,
                                                                        guid: "",
                                                                        type: this.lastSpendingType(this.state.tour)
                                                                    }}
                                                                ><Button color='primary' variant='outlined'
                                                                >Spend</Button></SpendingForm>
                                                            }
                                                        </TableCell>
                                                        <TableCell align="right">{p.weight}</TableCell>
                                                        <TableCell align="right">
                                                            <SpendingsDetail person={p} spendingInfo={p.spentSendingInfo} showText={p.spentInCents} open={false}
                                                                received={false} />

                                                        </TableCell>
                                                        <TableCell align="right">
                                                            <SpendingsDetail person={p} spendingInfo={p.receivedSendingInfo} showText={p.receivedInCents} open={false}
                                                                received={true} />
                                                        </TableCell>
                                                        <TableCell align="center" style={
                                                            (p.receivedInCents - p.spentInCents) == 0
                                                            ? {}
                                                            : { backgroundColor: (p.receivedInCents - p.spentInCents) <= 0 ? '#EAFAF1' : '#F9EBEA' }

                                                        }>
                                                            {(p.receivedInCents - p.spentInCents) != 0 ?
                                                                (p.receivedInCents - p.spentInCents) < 0
                                                                    ? <span style={{ color: "green", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                                    : <span style={{ color: "red", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                                : <span style={{ color: "black", fontWeight: "bold" }}>{(p.receivedInCents - p.spentInCents)}</span>
                                                            }
                                                        </TableCell>
                                                    </TableRow>
                                                ))}
                                            </TableBody>
                                        </Table>
                                    )} />
                                {/*--- /Persons ---*/}

                            </Switch>
                        </main>
                    </div>
                </Router>

            )
            //return <div><pre>Tour: {JSON.stringify(this.state.tour, null, 2)}}</pre></div>
        }
        function TPersons() {
            return <div> persons: <pre>{JSON.stringify(this.state.tour.persons, null, 2)}}</pre></div>
        }
        function TSpendings() {
            return <div> spendings: <pre>{JSON.stringify(this.state.tour.spendings, null, 2)}}</pre></div>
        }
    }

}

class TourRequestAccessCode extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false
        }
    }

    code = 'wrong_code'

    render() {
        return (
            <div>
                <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this.props.app, 'code', this.code)
                }}>
                    <p>access code:</p>
                    <input
                        type='text'
                        onChange={(e) => this.code = event.target.value}
                    />
                    <input
                        type='submit'
                    />
                </form>
            </div>
        )
    }

}