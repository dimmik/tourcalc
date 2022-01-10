import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';
import TourAdd from './tour-add.jsx'
import TourNameEdit from './tour-rename.jsx'

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableFooter from '@material-ui/core/TableFooter';
import TablePagination from '@material-ui/core/TablePagination';
import TableRow from '@material-ui/core/TableRow';
import Button from '@material-ui/core/Button';
import Grid from '@material-ui/core/Grid';
import TextField from '@material-ui/core/TextField';


export default class TourList extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isToursLoaded: false,
            tours: null,
            showArchived: false
        }
    }
    page = 0
    rowsPerPage = 15

    code = ""
    

    componentDidMount() {
        document.title = "Tourcalc: List of tours"
        this.loadTours()
    }
    loadTours() {
        return AppState.loadTours(this, this.page * this.rowsPerPage, this.rowsPerPage, this.code);
    }
    
    render() {
        if (!this.state.isToursLoaded) {
            return <div>Loading Tours ...</div>
        } else {
            return (
                <div>
                    <Table border={0} stickyHeader>
                        <TableHead>
                            <TableRow>
                                <TableCell>Tours
                                </TableCell>
                                <TableCell onClick={() => { /*alert('will refresh'); AppState.refreshMainApp();*/ }}>
                                    Mode: {this.props.authData.type}&nbsp;<Link to="/login">change</Link>
                                    {this.props.authData.type === 'Master'
                                        ? <div>
                                            <input type="text"
                                                defaultValue={this.code}
                                                onChange={(e) => { this.code = e.target.value; this.loadTours(); }} />
                                            {/*<Button variant="outlined" onClick={() => { this.loadTours() }}>filter</Button>*/}
                                        </div>
                                        : ''
                                    }
                                    a: <input
                                        type="checkbox"
                                        defaultChecked={this.state.showArchived}
                                        onChange={(e) => { this.setState({ showArchived: e.target.checked} ) }} />
                                </TableCell>
                                <TableCell>{
                                    this.props.authData.type === 'Master'
                                        ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={true}>
                                            <Button color='primary' variant='outlined'>Add Tour</Button>
                                        </TourAdd>
                                        : (
                                            this.state.tours.tours.length > 0
                                                ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={false}>
                                                    <Button color='primary' variant='outlined'>Add Tour</Button>
                                                </TourAdd>
                                                : <span />
                                        )
                                }</TableCell>
                                <TableCell>Clone
                                </TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {
                                this.state.tours.tours
                                    .filter(t => this.state.showArchived ? true : !t.isArchived)
                                    .map((t, idx) => {
                                    return (
                                        <TableRow key={'row' + t.id} hover>
                                            <TableCell>
                                            {
                                                /*this.props.authData.type === 'Master'*/ true ? (
                                                    <span key={'s' + idx} style={{ cursor: "pointer", borderStyle: 'ridge', fontSize: "xx-small" }} onClick={() => {
                                                        if (window.confirm('Sure to delete tour ' + t.name + ' (id: ' + t.id + ')?')) {
                                                            AppState.deleteTour(this, t.id)
                                                                .then(() => { this.loadTours() })
                                                        }
                                                    }}>X</span>) : <span />
                                                }
                                                <u key={'u' + idx}>
                                                    <TourNameEdit key={'te' + idx} tourid={t.id}
                                                        name={t.name} app={this}
                                                        open={false}
                                                        authData={this.props.authData}
                                                        buttonText='Rename' actionButtonText="Change name" /></u>
                                                <input type="checkbox" defaultChecked={t.isArchived}
                                                    onClick={() => {
                                                        AppState.changeTourArchived(this, t.id, !t.isArchived, t.code)
                                                        .then(() => { this.loadTours() }) }}
                                                />
                                            </TableCell>
                                            <TableCell>
                                                {(idx + 1) + (this.page * this.rowsPerPage)}.<Link key={'l' + idx} to={'/tour/' + t.id}>{t.name}
                                                </Link>&nbsp;
                                                [p<b>{t.persons.length}</b>;s<b>{t.spendings.filter(s => !s.planned).length}</b>;<b>{t.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length > 0 ? ((1 -
                                                    t.persons.filter(p => (p.receivedInCents - p.spentInCents) > 0).length * 1.0 /
                                                    t.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length)
                                                    * 100).toFixed(0) : 0
                                                }%</b>]
                                            </TableCell>
                                            <TableCell>
                                                <Button variant='outlined' onClick={() => {
                                                    document.getElementById('TourJsonTextArea').value = JSON.stringify(t, null, 2);
                                                }}>JSON</Button>
                                                <input type="text" id={t.id + "code"} label="Code" size="3" placeholder="code"
                                                    onChange={
                                                        () => document.getElementById(t.id + "link").href = "/goto/" + (document.getElementById(t.id + "code") == null ? "zzzz" : document.getElementById(t.id + "code").value) + "/" + t.id} />
                                                <a href="#" id={t.id + "link"} target="_blank">link</a>
                                                
                                            </TableCell>
                                            <TableCell>

                                                {this.props.authData.type === 'Master' ?
                                                    <TextField type="text" id={t.id + "tourAccessCode"} label="Access code" required />
                                                    : <input type="hidden" id={t.id + "tourAccessCode"} value='none'/>
                                                }
                                                <TextField type="text" id={t.id + "tourName"} label="New tour name" required />
                                                <Button color='secondary' variant='outlined' size="small" onClick={
                                                    () => {
                                                        //alert('id: ' + t.id + 'tourAccessCode' + ' val: ' + document.getElementById(t.id + 'tourAccessCode').value);
                                                        if (document.getElementById(t.id + 'tourAccessCode').value != '' && document.getElementById(t.id + 'tourName').value) {
                                                            AppState.addTourJson(this, t
                                                                , document.getElementById(t.id + 'tourAccessCode').value
                                                                , document.getElementById(t.id + 'tourName').value
                                                            )
                                                                .then(() => {
                                                                    this.loadTours()
                                                                })
                                                        } else {
                                                            alert('Please enter access code and tour name')
                                                        }
                                                    }
                                                }>Clone</Button>

                                            </TableCell>
                                  </TableRow>
                                    )
                                })
                            }
                        </TableBody>
                            <TableFooter>
                                <TableRow>
                                <TablePagination count={this.state.tours.totalCount}
                                    onChangePage={
                                        (e, p) => {
                                            //alert('p:' + p);
                                            this.page = p;
                                            this.loadTours()
                                        }
                                    }
                                    onChangeRowsPerPage={(e) => {
                                        //alert('r:' + e.target.value)
                                        this.rowsPerPage = e.target.value;
                                        this.page = 0
                                        this.loadTours()
                                    }}
                                    page={this.page} rowsPerPage={this.rowsPerPage}
                                    rowsPerPageOptions={[3, 7, 15, 25, 50]}
                                />
                                </TableRow>
                            </TableFooter>
                    </Table>

                    <hr />
                    Tour JSON:
                    <textarea id="TourJsonTextArea" style={{ width: "100%" }} rows="7" defaultValue="Here will be tour JSON"/>
                    <br/>New tour name: <input type="text" id="TourJsonName" defaultValue="New Tour"/>
                    <Button color='secondary' variant='outlined' size="small" onClick={
                                                    () => {
                                                        //alert('id: ' + t.id + 'tourAccessCode' + ' val: ' + document.getElementById(t.id + 'tourAccessCode').value);
                                                            AppState.addTourJson(this, JSON.parse(document.getElementById('TourJsonTextArea').value)
                                                                , 'մուտքի_կոդ'
                                                                , document.getElementById('TourJsonName').value
                                                            )
                                                                .then(() => {
                                                                    this.loadTours()
                                                                })
                                                    }
                                                }>Create</Button>
                </div>
                )
        }
    }

};